import fitz  # PyMuPDF
from pdf2image import convert_from_path
import pandas as pd
from PIL import Image, ImageDraw
import base64
from io import BytesIO
import json

def extract_pdf_page(pdf_path: str, page_num: int) -> tuple:
    """
    提取PDF指定页面的内容，分类处理文本、图像和表格
    
    参数:
        pdf_path: PDF文件路径
        page_num: 要提取的页码(1-based)
    
    返回:
        tuple: (页面内容的JSON字符串, 总页数)
    """
    try:
        doc = fitz.open(pdf_path)
        total_pages = len(doc)
        
        if page_num < 1 or page_num > total_pages:
            return (json.dumps({"error": f"Invalid page number {page_num}. Valid range is 1-{total_pages}"}), total_pages)
        
        page = doc.load_page(page_num - 1)
        
        # 获取页面缩略图
        thumbnail = get_page_thumbnail(pdf_path, page_num)
        
        # 提取内容元素
        elements = extract_page_elements(page)
        
        return (json.dumps({
            "page_number": page_num,
            "total_pages": total_pages,
            "thumbnail": thumbnail,
            "elements": elements
        }), total_pages)
        
    except Exception as e:
        return (json.dumps({"error": str(e)}), 0)

def get_page_thumbnail(pdf_path: str, page_num: int) -> str:
    """生成页面缩略图"""
    images = convert_from_path(pdf_path, first_page=page_num, last_page=page_num)
    if not images:
        return ""
    
    img_bytes = BytesIO()
    images[0].save(img_bytes, format='PNG')
    return base64.b64encode(img_bytes.getvalue()).decode('utf-8')

def extract_page_elements(page) -> list:
    """提取页面中的所有元素并分类处理"""
    elements = []
    
    # 1. 提取文本块（按段落）
    text_blocks = page.get_text("blocks")
    for block in text_blocks:
        if block[6] == 0:  # 文本块类型
            text = block[4].strip()
            if text:
                elements.append({
                    "type": "text",
                    "content": text,
                    "bbox": block[:4]
                })
    
    # 2. 提取图像
    for img in page.get_images():
        xref = img[0]
        base_image = page.parent.extract_image(xref)
        elements.append({
            "type": "image",
            "content": base64.b64encode(base_image["image"]).decode('utf-8'),
            "bbox": [0, 0, base_image["width"], base_image["height"]]  # 简化位置
        })
    
    # 3. 提取表格并转换为图像
    tables = page.find_tables()
    if tables.tables:
        for table in tables.tables:
            df = table.to_pandas()
            table_image = dataframe_to_image(df)
            
            img_bytes = BytesIO()
            table_image.save(img_bytes, format="PNG")
            img_str = base64.b64encode(img_bytes.getvalue()).decode('utf-8')
            
            elements.append({
                "type": "table",
                "content": img_str,
                "bbox": list(table.bbox),
                "data": df.to_dict(orient="records")
            })
    
    # 按大致阅读顺序排序（从上到下）
    elements.sort(key=lambda x: x["bbox"][1])
    
    return elements

def dataframe_to_image(df):
    """将DataFrame转换为美观的图像"""
    # 计算列宽
    col_widths = []
    for col in df.columns:
        max_len = max(df[col].astype(str).apply(len).max(), len(col))
        col_widths.append(min(200, max(80, max_len * 8)))
    
    # 计算图像大小
    cell_height = 40
    img_width = sum(col_widths)
    img_height = cell_height * (len(df) + 1)  # +1 for header
    
    # 创建图像
    img = Image.new('RGB', (img_width, img_height), color=(255, 255, 255))
    draw = ImageDraw.Draw(img)
    
    # 绘制表头
    x_pos = 0
    for i, col in enumerate(df.columns):
        draw.rectangle([x_pos, 0, x_pos + col_widths[i], cell_height], 
                      fill=(240, 240, 240), outline=(200, 200, 200))
        draw.text((x_pos + 10, 10), str(col), fill=(0, 0, 0))
        x_pos += col_widths[i]
    
    # 绘制表格内容
    for j in range(len(df)):
        x_pos = 0
        y_pos = (j + 1) * cell_height
        for i, col in enumerate(df.columns):
            draw.rectangle([x_pos, y_pos, x_pos + col_widths[i], y_pos + cell_height], 
                         fill=(255, 255, 255), outline=(200, 200, 200))
            cell_value = str(df.iloc[j, i])
            draw.text((x_pos + 10, y_pos + 10), cell_value, fill=(0, 0, 0))
            x_pos += col_widths[i]
    
    return img