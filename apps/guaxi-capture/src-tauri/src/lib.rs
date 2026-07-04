use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    time::Duration,
};
use tauri::{AppHandle, Emitter};

const CAPTURE_JSON_ARG: &str = "--capture-json";
const CAPTURE_TEXT_ARG: &str = "--capture-text";
const CAPTURE_URL_ARG: &str = "--capture-url";
const LOCAL_CAPTURE_RECEIVER_ADDR: &str = "127.0.0.1:14230";
const MAX_LOCAL_REQUEST_BYTES: usize = 2 * 1024 * 1024;

#[tauri::command]
fn read_startup_capture_payload() -> Result<Option<Value>, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let payload = parse_startup_capture_args(&args)?;

    if let Some(value) = &payload {
        println!(
            "[capture-native] startup payload detected: keys={}",
            value.as_object().map(|object| object.len()).unwrap_or_default()
        );
    } else {
        println!("[capture-native] no startup capture payload");
    }

    Ok(payload)
}

#[tauri::command]
fn capture_ocr_recognize(endpoint: String, image_base64: String) -> Result<Value, String> {
    if image_base64.trim().is_empty() {
        return Err("OCR 图片 base64 为空".to_string());
    }

    println!(
        "[capture-ocr] proxy request endpoint={} bytes={}",
        endpoint,
        image_base64.len()
    );

    let payload = json!({
        "image_path": "",
        "image_base64": image_base64,
    });

    post_json_to_http_endpoint(&endpoint, &payload)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            start_local_capture_receiver(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            read_startup_capture_payload,
            capture_ocr_recognize
        ])
        .run(tauri::generate_context!())
        .expect("error while running Guaxi Capture");
}

// 设计说明：
// 浏览器插件、脚本工具和后续桌面深链都可以先把采集内容投递到 Capture App，
// 由用户在 App 内确认后再发送到 Guaxi 主程序。这里用本地 HTTP 接收器降低接入门槛，
// 同时只监听 127.0.0.1，避免暴露到局域网。
fn start_local_capture_receiver(app_handle: AppHandle) {
    std::thread::spawn(move || {
        let listener = match TcpListener::bind(LOCAL_CAPTURE_RECEIVER_ADDR) {
            Ok(listener) => listener,
            Err(error) => {
                println!(
                    "[capture-receiver] bind failed addr={} error={}",
                    LOCAL_CAPTURE_RECEIVER_ADDR, error
                );
                return;
            }
        };

        println!(
            "[capture-receiver] listening addr={}",
            LOCAL_CAPTURE_RECEIVER_ADDR
        );

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let app_handle = app_handle.clone();
                    std::thread::spawn(move || handle_local_capture_stream(stream, app_handle));
                }
                Err(error) => {
                    println!("[capture-receiver] accept failed error={error}");
                }
            }
        }
    });
}

fn handle_local_capture_stream(mut stream: TcpStream, app_handle: AppHandle) {
    let request = match read_local_http_request(&mut stream) {
        Ok(request) => request,
        Err(error) => {
            println!("[capture-receiver] request read failed error={error}");
            let _ = write_json_response(&mut stream, 400, json!({
                "success": false,
                "message": error
            }));
            return;
        }
    };

    if request.method == "OPTIONS" {
        let _ = write_json_response(&mut stream, 204, json!({}));
        return;
    }

    if request.method != "POST" || !is_capture_receiver_path(&request.path) {
        println!(
            "[capture-receiver] unsupported request method={} path={}",
            request.method, request.path
        );
        let _ = write_json_response(&mut stream, 404, json!({
            "success": false,
            "message": "unsupported capture receiver path"
        }));
        return;
    }

    let payload = match serde_json::from_slice::<Value>(&request.body) {
        Ok(payload) => payload,
        Err(error) => {
            println!("[capture-receiver] invalid json error={error}");
            let _ = write_json_response(&mut stream, 400, json!({
                "success": false,
                "message": format!("invalid json: {error}")
            }));
            return;
        }
    };

    println!(
        "[capture-receiver] payload received path={} bytes={}",
        request.path,
        request.body.len()
    );

    if let Err(error) = app_handle.emit("guaxi-capture-incoming", payload) {
        println!("[capture-receiver] emit failed error={error}");
        let _ = write_json_response(&mut stream, 500, json!({
            "success": false,
            "message": format!("emit failed: {error}")
        }));
        return;
    }

    let _ = write_json_response(&mut stream, 200, json!({
        "success": true,
        "message": "capture payload accepted"
    }));
}

// 设计说明：
// 桌面端先通过启动参数模拟系统分享入口，移动端 Share Intent 接入后也会输出同样的 payload。
// 这样前端只维护一套 shareIntake 归一化逻辑，避免每个平台各自拼表单字段。
fn parse_startup_capture_args(args: &[String]) -> Result<Option<Value>, String> {
    let capture_json = read_arg_value(args, CAPTURE_JSON_ARG);
    if let Some(raw_json) = capture_json {
        let value = serde_json::from_str::<Value>(&raw_json)
            .map_err(|error| format!("启动参数 capture-json 不是有效 JSON: {error}"))?;
        return Ok(Some(value));
    }

    let capture_text = read_arg_value(args, CAPTURE_TEXT_ARG);
    if let Some(text) = capture_text {
        return Ok(Some(json!({
            "text": text,
            "sourceApp": "desktop"
        })));
    }

    let capture_url = read_arg_value(args, CAPTURE_URL_ARG);
    if let Some(url) = capture_url {
        return Ok(Some(json!({
            "sourceUrl": url,
            "sourceApp": "browser"
        })));
    }

    Ok(None)
}

fn read_arg_value(args: &[String], name: &str) -> Option<String> {
    let inline_prefix = format!("{name}=");
    for (index, arg) in args.iter().enumerate() {
        if let Some(value) = arg.strip_prefix(&inline_prefix) {
            return Some(value.to_string());
        }

        if arg == name {
            return args.get(index + 1).cloned();
        }
    }

    None
}

struct LocalHttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn read_local_http_request(stream: &mut TcpStream) -> Result<LocalHttpRequest, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| format!("set read timeout failed: {error}"))?;

    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];

    loop {
        let read_count = stream
            .read(&mut chunk)
            .map_err(|error| format!("read failed: {error}"))?;
        if read_count == 0 {
            break;
        }

        buffer.extend_from_slice(&chunk[..read_count]);
        if buffer.len() > MAX_LOCAL_REQUEST_BYTES {
            return Err("request too large".to_string());
        }

        if request_body_complete(&buffer)? {
            break;
        }
    }

    parse_local_http_request(&buffer)
}

fn request_body_complete(buffer: &[u8]) -> Result<bool, String> {
    let Some(header_end) = find_header_end(buffer) else {
        return Ok(false);
    };

    let header_text = String::from_utf8_lossy(&buffer[..header_end]);
    let content_length = parse_content_length(&header_text)?;
    Ok(buffer.len() >= header_end + 4 + content_length)
}

fn parse_local_http_request(buffer: &[u8]) -> Result<LocalHttpRequest, String> {
    let header_end = find_header_end(buffer).ok_or_else(|| "missing http header".to_string())?;
    let header_text = String::from_utf8_lossy(&buffer[..header_end]);
    let mut header_lines = header_text.lines();
    let request_line = header_lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| "missing method".to_string())?
        .to_string();
    let path = request_parts
        .next()
        .ok_or_else(|| "missing path".to_string())?
        .to_string();
    let content_length = parse_content_length(&header_text)?;
    let body_start = header_end + 4;
    let body_end = body_start + content_length;

    if buffer.len() < body_end {
        return Err("incomplete body".to_string());
    }

    Ok(LocalHttpRequest {
        method,
        path,
        body: buffer[body_start..body_end].to_vec(),
    })
}

fn parse_content_length(header_text: &str) -> Result<usize, String> {
    for line in header_text.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };

        if name.trim().eq_ignore_ascii_case("content-length") {
            return value
                .trim()
                .parse::<usize>()
                .map_err(|error| format!("invalid content-length: {error}"));
        }
    }

    Ok(0)
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn is_capture_receiver_path(path: &str) -> bool {
    let path_without_query = path.split('?').next().unwrap_or(path);
    matches!(path_without_query, "/capture" | "/capture/import")
}

fn write_json_response(stream: &mut TcpStream, status: u16, body: Value) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let body_text = if status == 204 {
        String::new()
    } else {
        body.to_string()
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nConnection: close\r\n\r\n{}",
        body_text.as_bytes().len(),
        body_text
    );

    stream.write_all(response.as_bytes())
}

fn post_json_to_http_endpoint(endpoint: &str, payload: &Value) -> Result<Value, String> {
    let target = parse_http_endpoint(endpoint)?;
    let body = payload.to_string();
    let addr = format!("{}:{}", target.host, target.port);
    let socket_addr = addr
        .to_socket_addrs()
        .map_err(|error| format!("解析 OCR 地址失败: {error}"))?
        .next()
        .ok_or_else(|| "OCR 地址为空".to_string())?;
    let mut stream = TcpStream::connect_timeout(&socket_addr, Duration::from_secs(5))
        .map_err(|error| format!("连接 OCR 服务失败: {error}"))?;

    stream
        .set_read_timeout(Some(Duration::from_secs(130)))
        .map_err(|error| format!("设置 OCR 读取超时失败: {error}"))?;

    let request = format!(
        "POST {} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        target.path,
        target.host,
        target.port,
        body.as_bytes().len(),
        body
    );

    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("发送 OCR 请求失败: {error}"))?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| format!("读取 OCR 响应失败: {error}"))?;

    parse_json_http_response(&response)
}

struct HttpEndpoint {
    host: String,
    port: u16,
    path: String,
}

fn parse_http_endpoint(endpoint: &str) -> Result<HttpEndpoint, String> {
    let raw = endpoint.trim();
    let without_scheme = raw
        .strip_prefix("http://")
        .ok_or_else(|| "OCR 接口当前仅支持 http:// 地址".to_string())?;
    let (host_port, path) = match without_scheme.split_once('/') {
        Some((host_port, path)) => (host_port, format!("/{}", path)),
        None => (without_scheme, "/".to_string()),
    };
    let (host, port) = match host_port.rsplit_once(':') {
        Some((host, port)) => {
            let parsed_port = port
                .parse::<u16>()
                .map_err(|error| format!("OCR 端口无效: {error}"))?;
            (host.to_string(), parsed_port)
        }
        None => (host_port.to_string(), 80),
    };

    if host.is_empty() {
        return Err("OCR host 为空".to_string());
    }

    Ok(HttpEndpoint { host, port, path })
}

fn parse_json_http_response(response: &[u8]) -> Result<Value, String> {
    let header_end = find_header_end(response).ok_or_else(|| "OCR 响应缺少 HTTP 头".to_string())?;
    let header_text = String::from_utf8_lossy(&response[..header_end]);
    let status_line = header_text
        .lines()
        .next()
        .ok_or_else(|| "OCR 响应状态行为空".to_string())?;
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "OCR 响应状态码为空".to_string())?
        .parse::<u16>()
        .map_err(|error| format!("OCR 响应状态码无效: {error}"))?;
    let body = &response[header_end + 4..];

    if !(200..300).contains(&status_code) {
        return Err(format!(
            "OCR 服务返回错误: HTTP {} {}",
            status_code,
            String::from_utf8_lossy(body)
        ));
    }

    serde_json::from_slice::<Value>(body).map_err(|error| format!("解析 OCR JSON 失败: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_inline_capture_json() {
        let args = vec![
            "--capture-json={\"title\":\"Demo\",\"markdown\":\"# Demo\"}".to_string(),
        ];

        let payload = parse_startup_capture_args(&args).unwrap().unwrap();

        assert_eq!(payload["title"], "Demo");
        assert_eq!(payload["markdown"], "# Demo");
    }

    #[test]
    fn parses_capture_url_pair() {
        let args = vec![
            "--capture-url".to_string(),
            "https://example.com/article".to_string(),
        ];

        let payload = parse_startup_capture_args(&args).unwrap().unwrap();

        assert_eq!(payload["sourceUrl"], "https://example.com/article");
        assert_eq!(payload["sourceApp"], "browser");
    }

    #[test]
    fn parses_capture_receiver_content_length() {
        let header = "POST /capture HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: 18";

        let content_length = parse_content_length(header).unwrap();

        assert_eq!(content_length, 18);
    }

    #[test]
    fn accepts_capture_receiver_paths_with_query() {
        assert!(is_capture_receiver_path("/capture"));
        assert!(is_capture_receiver_path("/capture/import?source=extension"));
        assert!(!is_capture_receiver_path("/other"));
    }

    #[test]
    fn parses_http_ocr_endpoint() {
        let endpoint = parse_http_endpoint("http://127.0.0.1:9876/ocr").unwrap();

        assert_eq!(endpoint.host, "127.0.0.1");
        assert_eq!(endpoint.port, 9876);
        assert_eq!(endpoint.path, "/ocr");
    }
}
