import type { LucideIcon } from 'lucide-react'
import {
  Activity,
  Bot,
  BriefcaseBusiness,
  FileSearch,
  FolderKanban,
  Gavel,
  Globe2,
  Landmark,
  MessagesSquare,
  ScanSearch,
  ShieldCheck,
  TimerReset,
  Users,
} from 'lucide-react'

export type Metric = {
  label: string
  value: string
  detail: string
  icon: LucideIcon
}

export type WorkflowStep = {
  title: string
  description: string
  tag: string
  icon: LucideIcon
}

export type CaseBoardItem = {
  id: string
  name: string
  stage: string
  lead: string
  jurisdiction: string
  evidenceCount: number
  timelineCount: number
  risk: 'Low' | 'Medium' | 'High'
  summary: string
  watchpoints: string[]
}

export type EvidenceItem = {
  title: string
  source: string
  language: string
  confidence: string
}

export type TimelineItem = {
  date: string
  title: string
  detail: string
}

export type ActivityItem = {
  actor: string
  action: string
  time: string
}

export const metrics: Metric[] = [
  {
    label: '在办案件',
    value: '24',
    detail: '7 个案件处于证据归档冲刺窗口',
    icon: BriefcaseBusiness,
  },
  {
    label: '双语证据块',
    value: '18.6k',
    detail: '中英对照已覆盖 92% 的核心材料',
    icon: Globe2,
  },
  {
    label: '关系图节点',
    value: '312',
    detail: '已建立当事人、机构与资金流关联',
    icon: Users,
  },
  {
    label: '审计可追溯率',
    value: '99.4%',
    detail: '导出、翻译与检索动作已落操作日志',
    icon: ShieldCheck,
  },
]

export const workflow: WorkflowStep[] = [
  {
    title: '案件接入',
    description: '统一录入案件、成员与保密分级，避免项目启动信息散落。',
    tag: 'Matter Intake',
    icon: FolderKanban,
  },
  {
    title: '证据解析',
    description: '对合同、聊天记录、发票、图片及录音进行结构化处理与检索索引。',
    tag: 'Evidence Parsing',
    icon: ScanSearch,
  },
  {
    title: '时间轴与人物图谱',
    description: '把零散事实转成可视化争议叙事，支持庭前梳理与内部复盘。',
    tag: 'Narrative Assembly',
    icon: Landmark,
  },
  {
    title: '法律协作与导出',
    description: '面向律师、法务与外部顾问生成统一口径的交付物。',
    tag: 'Delivery',
    icon: Gavel,
  },
]

export const caseBoard: CaseBoardItem[] = [
  {
    id: 'LG-24031',
    name: '跨境供应链争议',
    stage: '证据校核',
    lead: 'Mia Chen',
    jurisdiction: '上海 / 新加坡',
    evidenceCount: 146,
    timelineCount: 28,
    risk: 'High',
    summary: '焦点集中在补充协议签署时点、发票回款路径与 WhatsApp 沟通记录。',
    watchpoints: ['补充协议电子签时间差', '英文附件与中文正文定义不一致', '付款凭证存在第三方代付'],
  },
  {
    id: 'LG-24018',
    name: '投资退出责任核查',
    stage: '人物关系梳理',
    lead: 'David Zhou',
    jurisdiction: '北京',
    evidenceCount: 89,
    timelineCount: 17,
    risk: 'Medium',
    summary: '正在核实实际控制人对外口径是否与董事会纪要、审计底稿形成冲突。',
    watchpoints: ['董事会纪要版本冲突', '资金用途说明多次变更', '历史承诺函缺少附件'],
  },
  {
    id: 'LG-23977',
    name: '劳动合规专项排查',
    stage: '导出报告',
    lead: 'Irene Wu',
    jurisdiction: '深圳',
    evidenceCount: 54,
    timelineCount: 11,
    risk: 'Low',
    summary: '案件事实已基本闭合，当前重点是形成面向管理层与外部律所的双版本报告。',
    watchpoints: ['离职补偿测算口径统一', '录音材料需脱敏', '导出清单需补齐审批人'],
  },
]

export const evidenceByCase: Record<string, EvidenceItem[]> = {
  'LG-24031': [
    {
      title: '主采购合同与 3 份补充协议',
      source: 'PDF / bilingual OCR',
      language: '中英双语',
      confidence: '98%',
    },
    {
      title: 'WhatsApp 商务沟通摘录',
      source: 'ZIP / chat parse',
      language: '英文原文',
      confidence: '94%',
    },
    {
      title: '香港中转账户付款流水',
      source: 'XLSX / financial parser',
      language: '英文表头',
      confidence: '91%',
    },
  ],
  'LG-24018': [
    {
      title: '董事会纪要版本比对',
      source: 'DOCX / version diff',
      language: '中文',
      confidence: '97%',
    },
    {
      title: '审计底稿关键摘要',
      source: 'PDF / evidence chunk',
      language: '中英双语',
      confidence: '93%',
    },
    {
      title: '实际控制人承诺函',
      source: 'Email / attachment parse',
      language: '中文',
      confidence: '95%',
    },
  ],
  'LG-23977': [
    {
      title: '员工访谈录音转写',
      source: 'Audio / ASR',
      language: '中文',
      confidence: '96%',
    },
    {
      title: '人事制度与考勤记录',
      source: 'PDF + XLSX',
      language: '中文',
      confidence: '98%',
    },
    {
      title: '报告导出模板与审批流',
      source: 'Template / export center',
      language: '中文',
      confidence: '99%',
    },
  ],
}

export const timelineByCase: Record<string, TimelineItem[]> = {
  'LG-24031': [
    {
      date: '2026-05-02',
      title: '补充协议 v3 完成电子签',
      detail: '签署时间晚于首笔回款 48 小时，是当前争议的关键断点。',
    },
    {
      date: '2026-05-07',
      title: '香港账户发生第三方代付',
      detail: '与合同付款义务主体不一致，需要补充授权链说明。',
    },
    {
      date: '2026-05-13',
      title: '销售总监发出延期交货说明',
      detail: '沟通记录出现 “subject to revised annex” 表述，可能影响解释顺位。',
    },
  ],
  'LG-24018': [
    {
      date: '2026-04-18',
      title: '董事会纪要版本出现差异',
      detail: '两份纪要在对赌触发条款表述上存在删改痕迹。',
    },
    {
      date: '2026-04-26',
      title: '审计底稿补录资金用途',
      detail: '新增说明与早期投资人简报不一致，需比对披露责任。',
    },
    {
      date: '2026-05-11',
      title: '承诺函补充附件缺失',
      detail: '影响承诺范围边界，需要回溯邮件链和法务留档。',
    },
  ],
  'LG-23977': [
    {
      date: '2026-05-03',
      title: '完成员工访谈转写',
      detail: '已按敏感字段进行脱敏预处理，可直接纳入工作底稿。',
    },
    {
      date: '2026-05-12',
      title: '排班与考勤交叉核验完成',
      detail: '异常班次已定位，补偿测算口径趋于稳定。',
    },
    {
      date: '2026-05-20',
      title: '报告模板进入导出审批',
      detail: '区分管理层版与外部律所版，保留不同敏感字段层级。',
    },
  ],
}

export const activities: ActivityItem[] = [
  {
    actor: 'Case Bot',
    action: '完成 42 份 PDF 的双语块翻译与检索索引刷新',
    time: '4 分钟前',
  },
  {
    actor: 'Mia Chen',
    action: '将跨境供应链争议案件风险等级提升为 High',
    time: '12 分钟前',
  },
  {
    actor: 'Audit Trail',
    action: '记录导出时间轴报告与证据清单的审批闭环',
    time: '26 分钟前',
  },
]

export const commandCenter = [
  {
    label: '智能检索',
    description: '按案件、人物、证据块与时间节点进行多维召回。',
    icon: FileSearch,
  },
  {
    label: '对话协作',
    description: '让案件团队围绕同一证据上下文完成问答与批注。',
    icon: MessagesSquare,
  },
  {
    label: '策略助手',
    description: '对关键事实链、举证缺口与导出方案给出下一步建议。',
    icon: Bot,
  },
  {
    label: 'SLA 监控',
    description: '关注待处理案件、翻译队列与导出审批耗时。',
    icon: TimerReset,
  },
]

export const pulse = [
  {
    title: '待补强证据',
    value: '06',
    detail: '3 个案件缺少附件签署页或付款授权链。',
    icon: Activity,
  },
  {
    title: '待复核导出',
    value: '03',
    detail: '均涉及外部顾问交付，需要二次脱敏确认。',
    icon: ShieldCheck,
  },
]
