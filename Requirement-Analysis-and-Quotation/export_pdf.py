#!/usr/bin/env python3
"""
Markdown to PDF Batch Exporter
将目录下的所有 Markdown 文档导出为精美的 PDF 文件

用法:
    python export_pdf.py              # 导出所有 .md 文件
    python export_pdf.py 文件名.md     # 导出指定文件
    python export_pdf.py --list       # 列出可导出的文件
"""

import argparse
import sys
import re
from pathlib import Path
from datetime import datetime

import markdown
from weasyprint import HTML

# ============== 配置区域 ==============

# 工作目录
WORK_DIR = Path(__file__).parent

# 输出目录（与源文件同目录）
OUTPUT_DIR = WORK_DIR

# 需要忽略的文件
IGNORE_FILES = {"export_pdf.py", "README.md"}

# 文档元数据映射（用于自定义页眉页脚）
DOC_METADATA = {
    "项目方案与报价单": {
        "title": "法律认知引擎（首期）项目方案与报价单",
        "client": "成都辨本企业管理",
        "version": "灵活方案版 v1.2",
        "valid_days": "30 天",
    },
    "技术实现方案": {
        "title": "法律认知引擎（首期）技术实现方案",
        "client": "内部技术文档",
        "version": "v1.2",
        "valid_days": "永久",
    },
    "谈判技巧与价格策略": {
        "title": "项目谈判技巧与价格策略",
        "client": "内部培训文档",
        "version": "v1.0",
        "valid_days": "内部使用",
    },
    "示例合同": {
        "title": "软件开发合同（示例）",
        "client": "成都辨本企业管理",
        "version": "v1.0",
        "valid_days": "参考模板",
    },
}

# 联系信息
CONTACT_INFO = {
    "name": "王舟",
    "phone": "15378391447",
    "email": "main@mails.wedevs.org",
}

# ============== CSS 样式 ==============

CSS_STYLES = """
@page {
    size: A4;
    margin: 2.5cm 2cm;
    @bottom-right {
        content: "Page " counter(page) " of " counter(pages);
        font-size: 9pt;
        color: #666;
    }
}

* {
    box-sizing: border-box;
}

body {
    font-family: "Noto Sans SC", "PingFang SC", "Microsoft YaHei", sans-serif;
    font-size: 11pt;
    line-height: 1.8;
    color: #333;
    max-width: 210mm;
    margin: 0 auto;
}

h1 {
    font-size: 24pt;
    color: #1a365d;
    border-bottom: 3px solid #2c5282;
    padding-bottom: 12pt;
    margin-top: 0;
    margin-bottom: 20pt;
    text-align: center;
}

h2 {
    font-size: 16pt;
    color: #2c5282;
    border-bottom: 1px solid #e2e8f0;
    padding-bottom: 6pt;
    margin-top: 24pt;
    margin-bottom: 12pt;
}

h3 {
    font-size: 13pt;
    color: #2d3748;
    margin-top: 18pt;
    margin-bottom: 8pt;
}

h4 {
    font-size: 11pt;
    color: #4a5568;
    font-weight: 600;
    margin-top: 14pt;
    margin-bottom: 6pt;
}

h5 {
    font-size: 10pt;
    color: #64748b;
    font-weight: 600;
    margin-top: 12pt;
    margin-bottom: 4pt;
}

.header-info {
    background: linear-gradient(135deg, #1a365d 0%, #2c5282 100%);
    color: white;
    padding: 20pt;
    margin: -2.5cm -2cm 24pt -2cm;
    text-align: center;
}

.header-info h1 {
    color: white;
    border: none;
    margin: 0;
    padding: 0;
    font-size: 22pt;
}

.header-meta {
    margin-top: 12pt;
    font-size: 9pt;
    opacity: 0.9;
}

.header-meta span {
    margin: 0 8pt;
}

table {
    width: 100%;
    border-collapse: collapse;
    margin: 14pt 0;
    font-size: 10pt;
}

th, td {
    border: 1px solid #e2e8f0;
    padding: 8pt 10pt;
    text-align: left;
}

th {
    background: #f7fafc;
    font-weight: 600;
    color: #2d3748;
}

tr:nth-child(even) {
    background: #f8fafb;
}

tr:hover {
    background: #edf2f7;
}

strong {
    color: #1a365d;
}

code {
    background: #edf2f7;
    padding: 2pt 6pt;
    border-radius: 3pt;
    font-family: "Cascadia Code", "Fira Code", monospace;
    font-size: 9.5pt;
}

pre {
    background: #1a202c;
    color: #e2e8f0;
    padding: 12pt;
    border-radius: 6pt;
    overflow-x: auto;
    font-size: 9pt;
    line-height: 1.5;
}

pre code {
    background: none;
    padding: 0;
    color: inherit;
}

blockquote {
    border-left: 4pt solid #4299e1;
    margin: 14pt 0;
    padding: 8pt 14pt;
    background: #ebf8ff;
    color: #2c5282;
    font-style: italic;
}

ul, ol {
    padding-left: 20pt;
}

li {
    margin: 4pt 0;
}

.highlight {
    background: #ffffb8;
    padding: 2pt 4pt;
}

.warning {
    background: #fff5f5;
    border-left: 4pt solid #fc8181;
    padding: 10pt 14pt;
    margin: 14pt 0;
}

.warning strong {
    color: #c53030;
}

.contact-box {
    background: #f7fafc;
    border: 1px solid #e2e8f0;
    border-radius: 6pt;
    padding: 14pt;
    margin-top: 20pt;
}

.contact-box p {
    margin: 4pt 0;
}

hr {
    border: none;
    border-top: 2px solid #e2e8f0;
    margin: 24pt 0;
}

.footer {
    text-align: center;
    font-size: 9pt;
    color: #718096;
    margin-top: 30pt;
    padding-top: 14pt;
    border-top: 1px solid #e2e8f0;
}

.version-badge {
    display: inline-block;
    background: #4299e1;
    color: white;
    padding: 2pt 8pt;
    border-radius: 10pt;
    font-size: 8pt;
    margin-left: 8pt;
}

/* 代码块行号 */
.code-block {
    position: relative;
}

/* 表格响应式 */
@media (max-width: 768px) {
    table {
        font-size: 9pt;
    }
    th, td {
        padding: 4pt 6pt;
    }
}
"""

# ============== 函数定义 ==============


def find_markdown_files(directory: Path) -> list[Path]:
    """查找目录下所有 Markdown 文件"""
    md_files = []
    for file in directory.glob("*.md"):
        if file.name not in IGNORE_FILES and not file.name.startswith("."):
            md_files.append(file)
    return sorted(md_files)


def extract_metadata(md_content: str, file_name: str) -> dict:
    """从 Markdown 内容或预设配置中提取元数据"""
    # 尝试从文件名获取元数据
    base_name = file_name.replace(".md", "")

    # 优先使用预设配置
    if base_name in DOC_METADATA:
        meta = DOC_METADATA[base_name]
        return {
            "title": meta.get("title", base_name),
            "client": meta.get("client", ""),
            "version": meta.get("version", ""),
            "valid_days": meta.get("valid_days", ""),
            "date": datetime.now().strftime("%Y年%m月%d日"),
        }

    # 尝试从文件内容提取 YAML Front Matter
    date_match = re.search(r"日期[:：]\s*(\d{4}[-/]\d{1,2}[-/]\d{1,2})", md_content)
    version_match = re.search(r"版本[:：]\s*v?(\d+\.\d+)", md_content, re.IGNORECASE)

    # 尝试提取标题
    title_match = re.search(r"^#\s*(.+)$", md_content, re.MULTILINE)

    return {
        "title": title_match.group(1).strip() if title_match else base_name,
        "client": "",
        "version": version_match.group(1) if version_match else "v1.0",
        "valid_days": "",
        "date": date_match.group(1).replace("-", "年").replace("/", "年") + "日"
        if date_match
        else datetime.now().strftime("%Y年%m月%d日"),
    }


def convert_to_html(md_content: str) -> str:
    """将 Markdown 转换为 HTML"""
    return markdown.markdown(
        md_content,
        extensions=[
            "tables",
            "fenced_code",
            "toc",
            "nl2br",
            "sane_lists",
        ],
        output_format="html",
    )


def create_html_template(html_body: str, metadata: dict) -> str:
    """创建完整的 HTML 文档"""
    return f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{metadata["title"]}</title>
    <style>
        {CSS_STYLES}
    </style>
</head>
<body>
    <div class="header-info">
        <h1>{metadata["title"]}</h1>
        <div class="header-meta">
            <span>📅 {metadata["date"]}</span>
            <span>📋 {metadata["version"]}</span>
            <span>⏰ {metadata["valid_days"]}</span>
        </div>
    </div>
    
    {html_body}
    
    <div class="footer">
        <p>{metadata["client"]}</p>
        <p>开发负责人：{CONTACT_INFO["name"]} | 电话：{CONTACT_INFO["phone"]} | 邮箱：{CONTACT_INFO["email"]}</p>
    </div>
</body>
</html>"""


def export_pdf(md_path: Path, output_dir: Path | None = None) -> tuple[bool, str]:
    """
    将单个 Markdown 文件导出为 PDF

    Returns:
        (success, message)
    """
    try:
        # 读取 Markdown 文件
        md_content = md_path.read_text(encoding="utf-8")

        # 提取元数据
        metadata = extract_metadata(md_content, md_path.name)

        # 转换为 HTML
        html_body = convert_to_html(md_content)

        # 创建完整 HTML
        html_template = create_html_template(html_body, metadata)

        # 生成 PDF
        html_doc = HTML(string=html_template)

        # 确定输出路径
        if output_dir is None:
            output_dir = md_path.parent
        pdf_path = output_dir / md_path.with_suffix(".pdf").name

        # 写入 PDF
        html_doc.write_pdf(pdf_path)

        return (
            True,
            f"✅ {md_path.name} → {pdf_path.name} ({pdf_path.stat().st_size / 1024:.1f} KB)",
        )

    except Exception as e:
        return False, f"❌ {md_path.name} 失败：{str(e)}"


def list_available_files(files: list[Path]) -> None:
    """列出可导出的文件"""
    print(f"\n📁 工作目录：{WORK_DIR}")
    print(f"📄 找到 {len(files)} 个 Markdown 文件:\n")

    for i, file in enumerate(files, 1):
        size = file.stat().st_size / 1024
        pdf_exists = file.with_suffix(".pdf").exists()
        status = "✅ 已导出" if pdf_exists else "⏳ 未导出"
        print(f"  {i}. {file.name} ({size:.1f} KB) - {status}")

    print()


def main():
    """主函数"""
    parser = argparse.ArgumentParser(
        description="Markdown to PDF Batch Exporter",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
示例:
  python export_pdf.py              # 导出所有 .md 文件
  python export_pdf.py 文件名.md     # 导出指定文件
  python export_pdf.py --list       # 列出可导出的文件
  python export_pdf.py file1.md file2.md  # 导出多个文件
        """,
    )

    parser.add_argument(
        "files",
        nargs="*",
        help="要导出的 Markdown 文件名（留空则导出全部）",
    )

    parser.add_argument(
        "--list",
        "-l",
        action="store_true",
        help="列出所有可导出的文件",
    )

    parser.add_argument(
        "--output-dir",
        "-o",
        type=Path,
        default=None,
        help="PDF 输出目录（默认与源文件同目录）",
    )

    args = parser.parse_args()

    # 列出文件模式
    if args.list:
        files = find_markdown_files(WORK_DIR)
        list_available_files(files)
        return 0

    # 确定要导出的文件
    if args.files:
        # 用户指定了文件
        files = []
        for file_name in args.files:
            file_path = WORK_DIR / file_name
            if not file_path.exists():
                print(f"⚠️  警告：文件不存在 - {file_name}")
                continue
            if not file_path.suffix.lower() == ".md":
                print(f"⚠️  警告：不是 Markdown 文件 - {file_name}")
                continue
            files.append(file_path)
    else:
        # 导出全部
        files = find_markdown_files(WORK_DIR)

    if not files:
        print("❌ 没有找到可导出的 Markdown 文件")
        return 1

    # 批量导出
    print(f"\n🚀 开始导出 {len(files)} 个文件...\n")

    success_count = 0
    fail_count = 0

    for file in files:
        success, message = export_pdf(file, args.output_dir)
        print(message)
        if success:
            success_count += 1
        else:
            fail_count += 1

    # 汇总报告
    print(f"\n{'=' * 50}")
    print(f"📊 导出完成:")
    print(f"   ✅ 成功：{success_count} 个")
    print(f"   ❌ 失败：{fail_count} 个")
    print(f"   📁 输出目录：{args.output_dir or WORK_DIR}")
    print(f"{'=' * 50}\n")

    return 0 if fail_count == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
