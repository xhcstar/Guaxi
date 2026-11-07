# 模型处理:对选中文本做润色/精简/扩写/翻译/摘要/纠错。
# 通过本机 8888 的 /api/plugin/llm 调用用户在设置里配置的模型(密钥不出后端)。
# editor transform 契约:返回 {"text": 新文本}。
import json
import urllib.request

_ENDPOINT = "http://127.0.0.1:8888/api/plugin/llm"


def _call(system, prompt):
    body = json.dumps({"system": system, "prompt": prompt}, ensure_ascii=False).encode("utf-8")
    req = urllib.request.Request(_ENDPOINT, data=body, headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=180) as resp:
            data = json.loads(resp.read().decode("utf-8"))
    except Exception as e:
        return None, "调用本地模型失败:%s" % e
    if not data.get("success"):
        return None, str(data.get("message") or "模型调用失败")
    text = str(data.get("text") or "").strip()
    if not text:
        return None, "模型返回为空"
    return text, ""


def run(ctx):
    text = str(ctx.get("selection") or "").strip()
    if not text:
        return {"error": "请先选中要处理的文本"}
    args = ctx.get("args") or {}
    mode = str(args.get("mode") or "润色")
    lang = str(args.get("target_lang") or "英文").strip() or "英文"
    extra = str(args.get("instruction") or "").strip()

    system = "你是严谨的中文写作与编辑助手。只输出处理后的正文,不要解释、不要加前缀。"
    if mode == "翻译":
        prompt = "请把下面的文本翻译成%s,保持原有 Markdown 结构和专有名词,只输出译文:\n\n%s" % (lang, text)
    elif mode == "摘要":
        prompt = "请用中文为下面文本写一段不超过 150 字的摘要,只输出摘要:\n\n%s" % text
    elif mode == "精简":
        prompt = "请在不丢失关键信息的前提下精简下面文本,只输出精简后的正文:\n\n%s" % text
    elif mode == "扩写":
        prompt = "请在保持原意的前提下扩写下面文本,补充必要的过渡与细节,只输出扩写后的正文:\n\n%s" % text
    elif mode == "纠错":
        prompt = "请修正下面文本的错别字、标点和语病,尽量不改变原句,只输出修正后的正文:\n\n%s" % text
    else:
        prompt = "请润色下面文本,使其通顺、准确、书面化,不改变事实与结构,只输出润色后的正文:\n\n%s" % text
    if extra:
        prompt += "\n\n额外要求:" + extra
    out, err = _call(system, prompt)
    if err:
        return {"error": err}
    return {"text": out}
