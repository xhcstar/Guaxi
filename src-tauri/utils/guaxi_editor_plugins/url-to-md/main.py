# 链接转 Markdown:抓取选中的网址,用 trafilatura 抽正文并转成 Markdown。
import re
import urllib.request

_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36"


def _bool(v, default=False):
    if isinstance(v, bool):
        return v
    if v is None:
        return default
    return str(v).lower() not in ("false", "0", "no", "")


def _fetch(url):
    try:
        import httpx
        with httpx.Client(follow_redirects=True, timeout=30.0, headers={"User-Agent": _UA}) as client:
            resp = client.get(url)
            resp.raise_for_status()
            return resp.text, str(resp.url)
    except Exception:
        req = urllib.request.Request(url, headers={"User-Agent": _UA})
        with urllib.request.urlopen(req, timeout=30) as resp:
            charset = resp.headers.get_content_charset() or "utf-8"
            return resp.read().decode(charset, "ignore"), url


def _strip_html(html):
    html = re.sub(r"(?is)<(script|style|noscript)[^>]*>.*?</\1>", " ", html)
    text = re.sub(r"(?s)<[^>]+>", "\n", html)
    text = re.sub(r"[ \t]+", " ", text)
    text = re.sub(r"\n{3,}", "\n\n", text)
    return text.strip()


def run(ctx):
    raw = str(ctx.get("selection") or "").strip()
    m = re.match(r"^<?(https?://[^\s<>]+?)>?$", raw)
    if not m:
        return {"error": "请先选中一个 http/https 链接"}
    url = m.group(1)
    args = ctx.get("args") or {}
    keep_links = _bool(args.get("keep_links"), True)
    keep_images = _bool(args.get("keep_images"), False)
    try:
        max_chars = int(float(args.get("max_chars") or 20000))
    except Exception:
        max_chars = 20000
    try:
        html, final_url = _fetch(url)
    except Exception as e:
        return {"error": "抓取失败:%s" % e}

    md = ""
    title = ""
    try:
        import trafilatura
        md = trafilatura.extract(
            html,
            url=final_url,
            output_format="markdown",
            include_links=keep_links,
            include_images=keep_images,
            include_comments=False,
            include_tables=True,
        ) or ""
        meta = trafilatura.extract_metadata(html, default_url=final_url)
        if meta is not None:
            title = (meta.title or "").strip()
    except Exception:
        md = ""
    if not md.strip():
        md = _strip_html(html)
    md = md.strip()
    if max_chars > 0 and len(md) > max_chars:
        md = md[:max_chars].rstrip() + "\n\n> 已截断,原文共 %d 字" % len(md)
    head = ("# %s\n\n" % title) if title else ""
    return {"text": head + "> 来源:<%s>\n\n" % final_url + md + "\n"}
