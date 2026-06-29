"""
Guaxi Agent Server - 网络工具
"""
import json
from . import register


@register(
    name="web_search",
    description="使用 DuckDuckGo 搜索网络信息。返回标题、URL 和摘要。用于查找最新信息、资料。",
    parameters={
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "搜索关键词"
            },
            "max_results": {
                "type": "integer",
                "description": "最大返回结果数，默认 5"
            }
        },
        "required": ["query"]
    }
)
async def web_search(args: dict, work_dir: str) -> str:
    query = args["query"]
    max_results = args.get("max_results", 5)
    
    try:
        from duckduckgo_search import DDGS
        with DDGS() as ddgs:
            results = list(ddgs.text(query, max_results=max_results))
        
        items = []
        for r in results:
            items.append({
                "title": r.get("title", ""),
                "url": r.get("href", ""),
                "snippet": r.get("body", ""),
            })
        
        return json.dumps({"query": query, "results": items, "count": len(items)}, ensure_ascii=False)
    except Exception as e:
        return json.dumps({"error": f"搜索失败: {e}"}, ensure_ascii=False)


@register(
    name="fetch_webpage",
    description="抓取网页内容并提取正文（支持 HTML 转 Markdown）。用于阅读文章、获取详细信息。",
    parameters={
        "type": "object",
        "properties": {
            "url": {
                "type": "string",
                "description": "网页 URL"
            },
            "max_chars": {
                "type": "integer",
                "description": "最大返回字符数，默认 8000"
            }
        },
        "required": ["url"]
    }
)
async def fetch_webpage(args: dict, work_dir: str) -> str:
    url = args["url"]
    max_chars = args.get("max_chars", 8000)
    
    try:
        import httpx
        from markdownify import markdownify as md
        
        async with httpx.AsyncClient(timeout=30.0, follow_redirects=True) as client:
            resp = await client.get(url, headers={
                "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
            })
            resp.raise_for_status()
            html = resp.text
        
        # 提取正文为 Markdown
        text = md(html, heading_style="ATX", strip=["script", "style", "nav", "footer", "header"])
        text = text.strip()
        
        total = len(text)
        truncated = total > max_chars
        if truncated:
            text = text[:max_chars]
        
        return json.dumps({
            "url": url,
            "content_type": resp.headers.get("content-type", "unknown"),
            "total_chars": total,
            "truncated": truncated,
            "content": text,
        }, ensure_ascii=False)
    except Exception as e:
        return json.dumps({"error": f"抓取失败: {e}"}, ensure_ascii=False)
