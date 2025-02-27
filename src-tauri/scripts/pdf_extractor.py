import fitz  # PyMuPDF

def extract_pdf_page(filepath: str, curpage: int) -> str:
    """
    提取 PDF 文件中指定页的内容。

    Args:
        filepath (str): PDF 文件路径。
        curpage (int): 当前页码（从 1 开始）。

    Returns:
        str: 提取的页面内容。
    """
    try:
        # 打开 PDF 文件
        doc = fitz.open(filepath)
        # 加载指定页面（页码从 0 开始）
        page = doc.load_page(curpage - 1)
        # 提取页面文本内容
        content = page.get_text()
        return content
    except Exception as e:
        raise RuntimeError(f"Failed to extract PDF content: {e}")