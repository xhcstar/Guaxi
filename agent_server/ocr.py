"""离线 OCR：基于 rapidocr（PP-OCR 模型的 ONNX 封装，统一包，支持 Python 3.13）。

依赖 rapidocr>=3.0.0 + onnxruntime。旧的 rapidocr-onnxruntime 封顶 Python 3.12，
在托管的 3.13 运行时无法安装，故改用新版统一包。

注意：新版模型不随 wheel 内置，首次识别时会按需下载并缓存到 ~/.rapidocr/models/
（需一次联网），之后离线可用。引擎首次加载较慢，故以懒加载单例复用。

新版 API 与旧版不同：
- 导入：from rapidocr import RapidOCR
- 返回：RapidOCROutput 数据类（或 None），字段为 .boxes / .txts / .scores，
  而非旧版的 [(box, text, score), ...] 元组列表。
"""
import base64
import threading

_engine = None
_lock = threading.Lock()


def _get_engine():
    """懒加载 RapidOCR 单例（双检锁，线程安全）。"""
    global _engine
    if _engine is None:
        with _lock:
            if _engine is None:
                from rapidocr import RapidOCR
                _engine = RapidOCR()  # 首次加载/下载模型，之后复用
    return _engine


def run_ocr(image_base64: str = "", image_path: str = "") -> dict:
    """识别图片文字。

    image_base64 与 image_path 二选一；base64 兼容 `data:image/png;base64,` 前缀。
    返回 {"lines": [{"text","confidence","box"}], "text": "整段文字"}。
    """
    engine = _get_engine()

    # RapidOCR 的 __call__ 接受 str/Path 路径、bytes、ndarray；直接喂即可。
    if image_base64:
        payload = image_base64.split(",", 1)[-1]  # 去掉可能的 data URL 前缀
        src = base64.b64decode(payload)
    elif image_path:
        src = image_path
    else:
        return {"lines": [], "text": ""}

    result = engine(src)

    # 新版：无文字时 result 可能为 None，或 result.boxes 为 None。
    if result is None or getattr(result, "boxes", None) is None:
        return {"lines": [], "text": ""}

    boxes = result.boxes
    txts = result.txts or ()
    scores = result.scores or ()

    lines = []
    for box, text, score in zip(boxes, txts, scores):
        # box 为 np.ndarray(shape=(4,2))，转成纯 Python 列表以便 JSON 序列化。
        box_list = box.tolist() if hasattr(box, "tolist") else [[float(x), float(y)] for x, y in box]
        lines.append({
            "text": text,
            "confidence": float(score),
            "box": box_list,
        })

    return {"lines": lines, "text": "\n".join(item["text"] for item in lines)}
