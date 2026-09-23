//! 音频流代理：给带防盗链(Referer 校验)的直链补上 Referer/UA，边下边喂前端 `<audio>`。
//!
//! 挂在既有 loopback actix 服务(127.0.0.1:8888)上，不新开端口。
//! 前端把解灰/网易云拿到的直链换成
//! `http://127.0.0.1:8888/api/music/stream?url=<urlencoded>` 再喂给 `<audio>`。
//! 透传 Range 以支持拖动进度(seek)。

use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use serde::Deserialize;
use tracing::warn;

static UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36";

static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent(UA)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

#[derive(Deserialize)]
struct StreamQuery {
    url: String,
    referer: Option<String>,
}

// 按上游 host 猜一个合适的 Referer（各站防盗链只校验 Referer 域名）。
fn guess_referer(url: &str) -> &'static str {
    let u = url.to_ascii_lowercase();
    if u.contains("bilivideo") || u.contains("bilibili") {
        "https://www.bilibili.com/"
    } else if u.contains("kuwo") {
        "https://www.kuwo.cn/"
    } else if u.contains("kugou") {
        "https://www.kugou.com/"
    } else if u.contains("migu") {
        "https://music.migu.cn/"
    } else {
        // 网易云 music.126.net 等
        "https://music.163.com/"
    }
}

async fn music_stream(req: HttpRequest, q: web::Query<StreamQuery>) -> HttpResponse {
    let referer = q
        .referer
        .clone()
        .unwrap_or_else(|| guess_referer(&q.url).to_string());

    let mut up = CLIENT.get(&q.url).header("Referer", referer);
    // 透传客户端的 Range，支持 <audio> 拖动进度。
    if let Some(range) = req.headers().get("range") {
        up = up.header("Range", range.clone());
    }

    let resp = match up.send().await {
        Ok(r) => r,
        Err(e) => {
            warn!("music stream 上游请求失败: {e}");
            return HttpResponse::BadGateway().body(format!("upstream error: {e}"));
        }
    };

    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::OK);
    let mut builder = HttpResponse::build(status);
    for h in ["content-type", "content-length", "content-range", "accept-ranges"] {
        if let Some(v) = resp.headers().get(h) {
            builder.insert_header((h, v.clone()));
        }
    }
    // 上游没声明可分段时，明确告诉浏览器支持按字节续传。
    if resp.headers().get("accept-ranges").is_none() {
        builder.insert_header(("accept-ranges", "bytes"));
    }

    let stream = resp
        .bytes_stream()
        .map(|chunk| chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
    builder.streaming(stream)
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/music/stream", web::get().to(music_stream));
}
