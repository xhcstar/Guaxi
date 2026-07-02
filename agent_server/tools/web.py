"""
Guaxi Agent Server - 网络工具
"""
import asyncio
import json
import os
import time
from . import register


def _normalize_search_item(title: str = "", url: str = "", snippet: str = "") -> dict:
    """统一不同搜索提供方的返回格式，减少 Agent 侧需要理解的分支。"""
    return {
        "title": str(title or "").strip(),
        "url": str(url or "").strip(),
        "snippet": str(snippet or "").strip(),
    }


def _dedupe_search_items(items: list[dict], max_results: int) -> list[dict]:
    """按 URL 去重并过滤空结果，保证返回给模型的是紧凑可引用的数据。"""
    seen: set[str] = set()
    out: list[dict] = []
    for item in items:
        url = item.get("url", "").strip()
        title = item.get("title", "").strip()
        if not url or not title or url in seen:
            continue
        seen.add(url)
        out.append(item)
        if len(out) >= max_results:
            break
    return out


async def _search_tavily(query: str, max_results: int) -> list[dict]:
    """可选 Tavily API 搜索；用户配置 TAVILY_API_KEY 后自动启用。"""
    api_key = os.environ.get("TAVILY_API_KEY", "").strip()
    if not api_key:
        return []

    import httpx

    async with httpx.AsyncClient(timeout=20.0) as client:
        resp = await client.post(
            "https://api.tavily.com/search",
            json={
                "api_key": api_key,
                "query": query,
                "max_results": max_results,
                "include_answer": False,
                "include_raw_content": False,
            },
        )
        resp.raise_for_status()
        payload = resp.json()

    return [
        _normalize_search_item(
            title=item.get("title", ""),
            url=item.get("url", ""),
            snippet=item.get("content", ""),
        )
        for item in payload.get("results", [])
    ]


async def _search_brave(query: str, max_results: int) -> list[dict]:
    """可选 Brave Search API；比页面抓取稳定，适合作为联网搜索的生产 provider。"""
    api_key = os.environ.get("BRAVE_SEARCH_API_KEY", "").strip()
    if not api_key:
        return []

    import httpx

    async with httpx.AsyncClient(timeout=20.0) as client:
        resp = await client.get(
            "https://api.search.brave.com/res/v1/web/search",
            params={"q": query, "count": max_results},
            headers={
                "Accept": "application/json",
                "X-Subscription-Token": api_key,
            },
        )
        resp.raise_for_status()
        payload = resp.json()

    web_items = payload.get("web", {}).get("results", [])
    return [
        _normalize_search_item(
            title=item.get("title", ""),
            url=item.get("url", ""),
            snippet=item.get("description", ""),
        )
        for item in web_items
    ]


def _search_ddgs_sync(query: str, max_results: int) -> list[dict]:
    """使用 ddgs 搜索；兼容旧环境中的 duckduckgo_search，降低升级窗口期风险。"""
    try:
        from ddgs import DDGS
    except Exception:
        from duckduckgo_search import DDGS

    with DDGS() as ddgs:
        results = list(ddgs.text(query, max_results=max_results))

    return [
        _normalize_search_item(
            title=r.get("title", ""),
            url=r.get("href") or r.get("url", ""),
            snippet=r.get("body") or r.get("snippet", ""),
        )
        for r in results
    ]


@register(
    name="web_search",
    description="在用户手动开启联网后搜索网络信息。返回标题、URL 和摘要。用于查找最新信息、资料。",
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
    query = str(args["query"]).strip()
    max_results = max(1, min(int(args.get("max_results", 5) or 5), 10))
    started = time.perf_counter()
    errors: list[str] = []

    # 设计说明：搜索 provider 按“稳定 API -> 免费 fallback”排列。API key 不存在时直接跳过，
    # 因此默认仍保持开箱可用；配置 key 后则优先走更稳定的服务。
    providers = [
        ("tavily", lambda: _search_tavily(query, max_results)),
        ("brave", lambda: _search_brave(query, max_results)),
        ("ddgs", lambda: asyncio.to_thread(_search_ddgs_sync, query, max_results)),
    ]

    print(f"[agent_server] web_search start query_len={len(query)} max_results={max_results}", flush=True)
    for provider, runner in providers:
        provider_started = time.perf_counter()
        try:
            items = _dedupe_search_items(await runner(), max_results)
            duration_ms = int((time.perf_counter() - provider_started) * 1000)
            print(
                f"[agent_server] web_search provider={provider} "
                f"count={len(items)} duration_ms={duration_ms}",
                flush=True,
            )
            if items:
                total_ms = int((time.perf_counter() - started) * 1000)
                return json.dumps({
                    "query": query,
                    "provider": provider,
                    "results": items,
                    "count": len(items),
                    "duration_ms": total_ms,
                    "errors": errors,
                }, ensure_ascii=False)
        except Exception as e:
            msg = f"{provider}: {e}"
            errors.append(msg)
            print(f"[agent_server] web_search provider_failed {msg}", flush=True)

    total_ms = int((time.perf_counter() - started) * 1000)
    return json.dumps({
        "query": query,
        "results": [],
        "count": 0,
        "duration_ms": total_ms,
        "errors": errors,
        "error": "所有搜索 provider 均未返回可用结果",
    }, ensure_ascii=False)


@register(
    name="fetch_webpage",
    description="在用户手动开启联网后抓取网页内容并提取正文（支持 HTML 转 Markdown）。用于阅读文章、获取详细信息。",
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
    started = time.perf_counter()
    print(f"[agent_server] fetch_webpage start url={url} max_chars={max_chars}", flush=True)
    
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
        
        duration_ms = int((time.perf_counter() - started) * 1000)
        print(
            f"[agent_server] fetch_webpage done url={url} "
            f"total_chars={total} truncated={truncated} duration_ms={duration_ms}",
            flush=True,
        )
        return json.dumps({
            "url": url,
            "content_type": resp.headers.get("content-type", "unknown"),
            "total_chars": total,
            "truncated": truncated,
            "content": text,
        }, ensure_ascii=False)
    except Exception as e:
        print(f"[agent_server] fetch_webpage failed url={url} error={e}", flush=True)
        return json.dumps({"error": f"抓取失败: {e}"}, ensure_ascii=False)
