#!/usr/bin/env python3
"""
Markdown to PowerPoint Converter - 简化版
将投资人演示文稿 Markdown 转换为 PPT
"""

from pathlib import Path
from pptx import Presentation
from pptx.util import Inches, Pt
from pptx.enum.text import PP_ALIGN
from pptx.dml.color import RGBColor

# 颜色定义
PRIMARY_COLOR = RGBColor(26, 54, 93)  # #1a365d
SECONDARY_COLOR = RGBColor(44, 82, 130)  # #2c5282
ACCENT_COLOR = RGBColor(245, 158, 11)  # #f59e0b
TEXT_COLOR = RGBColor(33, 33, 33)  # #212121
WHITE = RGBColor(255, 255, 255)  # #ffffff


def create_presentation():
    """创建演示文稿"""
    prs = Presentation()
    prs.slide_width = Inches(13.333)
    prs.slide_height = Inches(7.5)
    return prs


def add_title_slide(prs, title, subtitle):
    """添加标题幻灯片"""
    slide = prs.slides.add_slide(prs.slide_layouts[0])

    title_shape = slide.shapes.title
    title_shape.text = title
    tf = title_shape.text_frame
    p = tf.paragraphs[0]
    p.font.bold = True
    p.font.size = Pt(44)
    p.font.color.rgb = PRIMARY_COLOR
    p.alignment = PP_ALIGN.CENTER

    subtitle_shape = slide.placeholders[1]
    subtitle_shape.text = subtitle
    tf = subtitle_shape.text_frame
    p = tf.paragraphs[0]
    p.font.size = Pt(24)
    p.font.color.rgb = SECONDARY_COLOR
    p.alignment = PP_ALIGN.CENTER


def add_section_slide(prs, section_num, section_title):
    """添加章节幻灯片"""
    slide = prs.slides.add_slide(prs.slide_layouts[6])

    # 背景
    bg = slide.shapes.add_shape(
        1,  # Rectangle
        0,
        0,
        prs.slide_width,
        prs.slide_height,
    )
    bg.fill.solid()
    bg.fill.fore_color.rgb = PRIMARY_COLOR
    bg.line.fill.background()

    # 章节号
    num_box = slide.shapes.add_textbox(Inches(1), Inches(2), Inches(3), Inches(2))
    tf = num_box.text_frame
    tf.text = section_num
    p = tf.paragraphs[0]
    p.font.bold = True
    p.font.size = Pt(72)
    p.font.color.rgb = WHITE
    p.alignment = PP_ALIGN.CENTER

    # 章节标题
    title_box = slide.shapes.add_textbox(Inches(4.5), Inches(3), Inches(8), Inches(2))
    tf = title_box.text_frame
    tf.text = section_title
    p = tf.paragraphs[0]
    p.font.size = Pt(36)
    p.font.color.rgb = WHITE


def add_bullet_slide(prs, title, bullets):
    """添加列表幻灯片"""
    slide = prs.slides.add_slide(prs.slide_layouts[1])

    title_shape = slide.shapes.title
    title_shape.text = title
    tf = title_shape.text_frame
    p = tf.paragraphs[0]
    p.font.bold = True
    p.font.size = Pt(32)
    p.font.color.rgb = PRIMARY_COLOR

    body_shape = slide.placeholders[1]
    tf = body_shape.text_frame
    tf.word_wrap = True

    for i, bullet in enumerate(bullets):
        if i == 0:
            p = tf.paragraphs[0]
        else:
            p = tf.add_paragraph()
        p.text = "• " + bullet
        p.font.size = Pt(20)
        p.font.color.rgb = TEXT_COLOR
        p.space_after = Pt(12)


def add_table_slide(prs, title, headers, rows):
    """添加表格幻灯片"""
    slide = prs.slides.add_slide(prs.slide_layouts[5])

    # 标题
    title_box = slide.shapes.add_textbox(
        Inches(0.5), Inches(0.3), Inches(12), Inches(1)
    )
    tf = title_box.text_frame
    tf.text = title
    p = tf.paragraphs[0]
    p.font.bold = True
    p.font.size = Pt(32)
    p.font.color.rgb = PRIMARY_COLOR

    # 表格
    table = slide.shapes.add_table(
        len(rows) + 1,
        len(headers),
        Inches(0.5),
        Inches(1.5),
        Inches(12.3),
        Inches(1 + (len(rows) + 1) * 0.7),
    ).table

    # 列宽
    col_width = Inches(12.3 / len(headers))
    for i in range(len(headers)):
        table.columns[i].width = col_width

    # 表头
    for i, header in enumerate(headers):
        cell = table.cell(0, i)
        cell.text = header
        cell.fill.solid()
        cell.fill.fore_color.rgb = PRIMARY_COLOR
        p = cell.text_frame.paragraphs[0]
        p.font.bold = True
        p.font.size = Pt(14)
        p.font.color.rgb = WHITE
        p.alignment = PP_ALIGN.CENTER

    # 数据行
    for row_idx, row in enumerate(rows, 1):
        for col_idx, cell_text in enumerate(row):
            cell = table.cell(row_idx, col_idx)
            cell.text = str(cell_text)
            p = cell.text_frame.paragraphs[0]
            p.font.size = Pt(13)
            p.font.color.rgb = TEXT_COLOR
            p.alignment = PP_ALIGN.CENTER
            if row_idx % 2 == 0:
                cell.fill.solid()
                cell.fill.fore_color.rgb = RGBColor(247, 250, 252)


def add_end_slide(prs, title, contact_lines):
    """添加结束幻灯片"""
    slide = prs.slides.add_slide(prs.slide_layouts[6])

    # 背景
    bg = slide.shapes.add_shape(1, 0, 0, prs.slide_width, prs.slide_height)
    bg.fill.solid()
    bg.fill.fore_color.rgb = PRIMARY_COLOR
    bg.line.fill.background()

    # 标题
    title_box = slide.shapes.add_textbox(Inches(2), Inches(2), Inches(9), Inches(2))
    tf = title_box.text_frame
    tf.text = title
    p = tf.paragraphs[0]
    p.font.bold = True
    p.font.size = Pt(48)
    p.font.color.rgb = WHITE
    p.alignment = PP_ALIGN.CENTER

    # 联系信息
    contact_box = slide.shapes.add_textbox(
        Inches(3), Inches(4.5), Inches(7), Inches(2.5)
    )
    tf = contact_box.text_frame
    tf.word_wrap = True

    for i, line in enumerate(contact_lines):
        if i == 0:
            p = tf.paragraphs[0]
        else:
            p = tf.add_paragraph()
        p.text = line
        p.font.size = Pt(18)
        p.font.color.rgb = WHITE
        p.alignment = PP_ALIGN.CENTER


def main():
    """主函数"""
    prs = create_presentation()

    # 1. 封面
    add_title_slide(
        prs, "法律认知引擎", "智能诉讼准备平台\n投资人演示文稿\n2026 年 3 月"
    )

    # 2. 目录
    add_bullet_slide(
        prs,
        "目录",
        [
            "项目概述",
            "市场痛点",
            "产品介绍",
            "商业模式",
            "竞争分析",
            "运营数据",
            "团队介绍",
            "融资计划",
        ],
    )

    # 3. 项目概述
    add_section_slide(prs, "01", "项目概述")

    add_bullet_slide(
        prs,
        "项目定位",
        [
            "AI 驱动的智能诉讼准备平台",
            "帮助律师快速整理证据、构建证据链、生成法庭材料",
            "让律师工作效率提升 10 倍",
        ],
    )

    add_table_slide(
        prs,
        "核心价值对比",
        ["传统方式", "法律认知引擎"],
        [
            ["人工整理证据 10 小时+", "AI 自动提取 1 小时"],
            ["容易遗漏关键证据", "智能关联无遗漏"],
            ["证据链手动构建", "可视化时间轴自动生成"],
            ["法庭材料手工整理", "一键导出标准格式"],
        ],
    )

    add_bullet_slide(
        prs,
        "愿景与使命",
        ["愿景：成为法律科技领域的领导者", "使命：用 AI 技术让法律服务更高效、更普惠"],
    )

    # 4. 市场痛点
    add_section_slide(prs, "02", "市场痛点")

    add_bullet_slide(
        prs,
        "律师的日常工作困境",
        [
            "证据整理耗时：一个案件 10-30 小时",
            "证据材料多：50-200 份文件",
            "人工成本高：3000-10000 元/案件",
            "错误风险高：容易遗漏关键证据",
        ],
    )

    add_table_slide(
        prs,
        "市场规模",
        ["指标", "数据"],
        [
            ["中国律师人数", "65 万+ (2025)"],
            ["年诉讼案件量", "3000 万+"],
            ["法律科技市场", "500 亿+"],
            ["年增长率", "25%+"],
        ],
    )

    # 5. 产品介绍
    add_section_slide(prs, "03", "产品介绍")

    add_bullet_slide(
        prs,
        "产品架构",
        [
            "输出层：证据清单 | 时间轴报告 | 案件分析",
            "画布层：可视化时间轴 + 双向锚定",
            "认知层⭐: AI 自动提取日期/金额/人物/事件",
            "摄入层：PDF | 图片 | 音频 | Excel | Word",
        ],
    )

    add_bullet_slide(
        prs,
        "核心功能",
        [
            "智能文件解析：支持 10+ 文件格式，OCR 准确率 97%+",
            "AI 信息提取：自动识别日期、金额、人物",
            "时间轴画布：可视化证据链，拖拽排序",
            "一键导出：标准证据清单、时间轴报告",
        ],
    )

    add_table_slide(
        prs,
        "产品优势",
        ["维度", "优势"],
        [
            ["技术壁垒", "LegalOne-R1 法律大模型 + 自研算法"],
            ["数据安全", "本地部署，数据不出服务器"],
            ["用户体验", "律师设计，符合工作习惯"],
            ["价格优势", "传统方案 1/3 价格"],
        ],
    )

    # 6. 商业模式
    add_section_slide(prs, "04", "商业模式")

    add_bullet_slide(
        prs,
        "收入来源",
        [
            "SaaS 订阅费 (60%)：基础版 999 元/月，专业版 2999 元/月，企业版 9999 元/月",
            "私有化部署 (30%)：15-50 万/套",
            "增值服务 (10%)：定制开发、培训服务",
        ],
    )

    add_table_slide(
        prs,
        "定价策略",
        ["版本", "价格", "目标客户", "功能"],
        [
            ["基础版", "999 元/月", "个人律师", "文件解析 + 时间轴"],
            ["专业版", "2999 元/月", "中小律所", "+AI 提取 + 协作"],
            ["企业版", "9999 元/月", "大型律所", "全功能 + 定制"],
            ["私有化", "15-50 万", "政府/央企", "本地部署"],
        ],
    )

    add_table_slide(
        prs,
        "单位经济模型",
        ["指标", "数值"],
        [
            ["CAC (获客成本)", "3000 元"],
            ["ARPU (月均收入)", "2500 元"],
            ["LTV (客户终身价值)", "75000 元"],
            ["LTV/CAC", "25:1"],
            ["回本周期", "2 个月"],
        ],
    )

    # 7. 竞争分析
    add_section_slide(prs, "05", "竞争分析")

    add_bullet_slide(
        prs,
        "核心竞争优势",
        [
            "技术优势：LegalOne-R1 法律大模型集成，自有专利算法",
            "市场优势：先入者优势，律师深度参与设计",
            "成本优势：AI 提效，人效比高，边际成本低",
        ],
    )

    # 8. 运营数据
    add_section_slide(prs, "06", "运营数据")

    add_table_slide(
        prs,
        "当前进展（2026 Q1）",
        ["指标", "数值"],
        [
            ["注册用户", "1,200+"],
            ["付费客户", "180+"],
            ["月经常性收入", "45 万"],
            ["月增长率", "25%+"],
            ["客户留存率", "92%"],
            ["NPS", "68"],
        ],
    )

    add_table_slide(
        prs,
        "财务预测",
        ["年份", "收入", "增长率", "利润"],
        [
            ["2026", "800 万", "-", "-200 万"],
            ["2027", "3000 万", "275%", "500 万"],
            ["2028", "8000 万", "167%", "2500 万"],
            ["2029", "1.8 亿", "125%", "6000 万"],
        ],
    )

    # 9. 团队介绍
    add_section_slide(prs, "07", "团队介绍")

    add_bullet_slide(
        prs,
        "创始人",
        [
            "王舟 - 创始人 & CEO",
            "10 年+ 软件开发经验",
            "前阿里巴巴高级技术专家",
            "法律科技连续创业者",
        ],
    )

    add_table_slide(
        prs,
        "核心团队",
        ["姓名", "职位", "背景"],
        [
            ["张三", "CTO", "前腾讯 AI 实验室负责人"],
            ["李四", "COO", "前华律网运营总监"],
            ["王五", "产品总监", "10 年法律产品设计经验"],
            ["赵六", "销售总监", "前法大大销售 VP"],
        ],
    )

    # 10. 融资计划
    add_section_slide(prs, "08", "融资计划")

    add_table_slide(
        prs,
        "融资需求",
        ["项目", "数值"],
        [
            ["融资金额", "1500 万"],
            ["出让股权", "10%"],
            ["投后估值", "1.5 亿"],
            ["资金用途", "产品研发 50% + 市场拓展 30% + 团队扩张 20%"],
        ],
    )

    add_bullet_slide(
        prs,
        "资金使用计划",
        [
            "产品研发 (50%)：AI 模型训练 400 万 + 功能开发 350 万",
            "市场拓展 (30%)：品牌推广 250 万 + 销售团队 200 万",
            "团队扩张 (20%)：核心人才引进 300 万",
        ],
    )

    add_bullet_slide(
        prs,
        "投资亮点",
        [
            "✅ 大市场：500 亿 + 法律科技市场",
            "✅ 强需求：律师效率痛点明显",
            "✅ 高壁垒：AI 技术 + 行业 Know-How",
            "✅ 快增长：月增长率 25%+",
            "✅ 好模式：SaaS + 私有化双轮驱动",
            "✅ 优团队：技术 + 法律 + 销售复合团队",
        ],
    )

    add_bullet_slide(
        prs,
        "退出路径",
        [
            "IPO 上市 (5-7 年)：科创板/创业板，预计估值 50 亿+",
            "并购退出 (3-5 年)：潜在收购方阿里/腾讯/法律垂直平台",
            "股权转让 (2-3 年)：后续轮次融资时部分退出",
        ],
    )

    # 11. 结束页
    add_end_slide(
        prs,
        "谢谢\nQ&A",
        [
            "期待与您合作",
            "",
            "公司：成都辨本企业管理",
            "联系人：王舟",
            "电话：15378391447",
            "邮箱：main@mails.wedevs.org",
        ],
    )

    # 保存
    output_path = str(Path(__file__).parent / "投资人演示文稿.pptx")
    prs.save(output_path)
    print(f"✅ PPT 生成成功：{output_path}")
    print(f"📊 幻灯片数量：{len(prs.slides)}")


if __name__ == "__main__":
    main()
