import pymupdf  # PyMuPDF
from operator import itemgetter
from itertools import groupby

# 缓存打开的 PDF 文档
pdf_cache = {}

def extract_pdf_page(filepath: str, curpage: int) -> tuple[str, int]:
    """
    提取 PDF 文件中指定页的内容，并返回总页数。

    Args:
        filepath (str): PDF 文件路径。
        curpage (int): 当前页码（从 1 开始）。

    Returns:
        tuple[str, int]: 提取的页面内容和总页数。
    """
    try:
        # 检查是否已经缓存了该文件
        if filepath not in pdf_cache:
            # 打开 PDF 文件并缓存
            pdf_cache[filepath] = pymupdf.open(filepath)
        
        # 获取缓存的文档
        doc = pdf_cache[filepath]
        totalpage = doc.page_count

        # 加载指定页面（页码从 0 开始）
        page = doc.load_page(curpage - 1)

        # 获取该页的所有单词（以单词为单位的坐标）
        text_words = page.get_text_words()

        # 按照 y1 (垂直位置) 和 x0 (水平位置) 排序
        sorted_words = sorted(text_words, key=itemgetter(3, 0))

        # 使用 groupby 按 y1 进行分组
        lines = groupby(sorted_words, key=itemgetter(3))

        # 遍历每一行，并输出每行的文本
        content = ""
        for y, line in lines:
            line_text = ' '.join(word[4] for word in line)  # 取出每个单词的文本部分（word[4]是文本）
            content += line_text + "\n"

        print(content)
        with open('D:/MyProject/Guaxi/Guaxi/log/pdftext.txt', 'w', encoding='utf-8') as pdf_file:
            pdf_file.write(content)
        return (content, totalpage)
    except Exception as e:
        raise RuntimeError(f"Failed to extract PDF content: {e}")