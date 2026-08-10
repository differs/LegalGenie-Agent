// Local Agent engine for the chat-first workspace.
//
// v1 is a deterministic rule engine (no LLM dependency): it parses the user's
// intent, executes read tools immediately and converts write intents into
// approval items that the user confirms in the right-hand queue before any
// mutation reaches the backend.
import * as api from './api'
import type {
  DedupeGroup,
  EvidenceFileSummary,
  PersonRelationship,
  SearchResult,
  TimelineNode,
} from './types'

// ---------- Cards ----------

export type PersonCardItem = {
  id: string
  name: string
  role_type: string
  phone?: string | null
  organization?: string | null
}

export type AgentCard =
  | { kind: 'nodes'; intro: string; nodes: TimelineNode[] }
  | { kind: 'files'; intro: string; files: EvidenceFileSummary[] }
  | { kind: 'persons'; intro: string; persons: PersonCardItem[] }
  | { kind: 'dedupe'; intro: string; groups: DedupeGroup[] }
  | { kind: 'relationships'; intro: string; relationships: PersonRelationship[] }
  | { kind: 'search'; intro: string; results: SearchResult[] }
  | { kind: 'info'; text: string }
  | { kind: 'error'; text: string }

export type ApprovalLevel = 'write' | 'destructive' | 'sensitive'

export type ApprovalItem = {
  id: string
  title: string
  summary: string
  level: ApprovalLevel
  // The mutation itself; confirmed by the user before it runs.
  execute: () => Promise<unknown>
  // Optional handler when the user rejects.
  onReject?: () => void
}

export type ChatMessage =
  | { role: 'user'; text: string; at: number }
  | {
      role: 'agent'
      text: string
      cards: AgentCard[]
      approvals: ApprovalItem[]
      options?: string[]
      at: number
    }

export type AgentContext = {
  token: string
  caseId?: string | null
  caseName?: string
  isReadOnly: boolean
  onNeedCase: () => void
  onUploadRequested: () => void
}

// ---------- Intent parsing ----------

type Intent =
  | { kind: 'help' }
  | { kind: 'list_cases' }
  | { kind: 'list_nodes' }
  | { kind: 'list_files' }
  | { kind: 'list_persons' }
  | { kind: 'dedupe' }
  | { kind: 'search'; keyword: string }
  | { kind: 'create_node'; title?: string; date?: string }
  | { kind: 'parse'; fileId?: string }
  | { kind: 'export_evidence_list' }
  | { kind: 'export_timeline_report'; format: 'pdf' | 'html' }
  | { kind: 'unknown'; text: string }

function parseIntent(text: string): Intent {
  const t = text.trim().toLowerCase()

  if (/\b(help|帮助|帮助|能做什么|怎么用)\b/.test(t) || t.startsWith('/help')) {
    return { kind: 'help' }
  }

  // /commands
  if (t.startsWith('/')) {
    const cmd = t.split(/\s+/)[0].slice(1)
    const rest = t.replace(/^\S+\s*/, '').trim()
    switch (cmd) {
      case 'cases':
        return { kind: 'list_cases' }
      case 'nodes':
      case 'timeline':
        return { kind: 'list_nodes' }
      case 'files':
      case 'evidence':
        return { kind: 'list_files' }
      case 'persons':
      case 'people':
        return { kind: 'list_persons' }
      case 'dedupe':
        return { kind: 'dedupe' }
      case 'search':
        return { kind: 'search', keyword: rest || '' }
      case 'export':
        return rest.includes('pdf')
          ? { kind: 'export_timeline_report', format: 'pdf' }
          : { kind: 'export_evidence_list' }
      case 'help':
        return { kind: 'help' }
      default:
        return { kind: 'unknown', text }
    }
  }

  const has = (re: RegExp) => re.test(t)

  if (has(/案件|case/)) return { kind: 'list_cases' }
  if (has(/去重|合并|重复|dedupe/)) return { kind: 'dedupe' }
  if (has(/人物|人员|person|people|关系图|关系/)) return { kind: 'list_persons' }
  if (has(/证据|文件|上传|材料|file|evidence|pdf|excel|xlsx/)) return { kind: 'list_files' }
  if (has(/时间轴|节点|timeline|node/)) return { kind: 'list_nodes' }
  if (has(/导出|export|报告|report|pdf/)) {
    if (has(/证据清单|evidence-list|xlsx/)) return { kind: 'export_evidence_list' }
    return { kind: 'export_timeline_report', format: has(/html/) ? 'html' : 'pdf' }
  }
  if (has(/搜索|查找|检索|search|find/)) {
    const keyword = t.replace(/搜索|查找|检索|search|find|搜一下|帮我找/g, '').trim()
    return { kind: 'search', keyword }
  }
  if (has(/新增|新建|创建|添加|add|create|new/)) {
    // 新建时间轴节点：抽取标题与日期
    const title = extractTitle(text)
    const date = extractDate(text)
    if (has(/节点|时间轴|timeline|node/)) {
      return { kind: 'create_node', title, date }
    }
  }

  return { kind: 'unknown', text }
}

function extractTitle(text: string): string | undefined {
  // 匹配引号/书名号内容，或"节点：xxx"模式
  const quoted = text.match(/[「“"']([^」”"']+)[」”"']/)
  if (quoted) return quoted[1].trim()
  const labeled = text.match(/(?:标题|名为|叫做|title)[:：\s]+([^\s，。,.;;]+)/)
  if (labeled) return labeled[1].trim()
  return undefined
}

function extractDate(text: string): string | undefined {
  const m = text.match(/(\d{4})[-/年](\d{1,2})[-/月](\d{1,2})日?/)
  if (m) {
    const y = m[1]
    const mo = m[2].padStart(2, '0')
    const d = m[3].padStart(2, '0')
    return `${y}-${mo}-${d}`
  }
  const md = text.match(/(\d{1,2})[-/月](\d{1,2})日?/)
  if (md) {
    const now = new Date()
    const year = now.getFullYear()
    return `${year}-${md[1].padStart(2, '0')}-${md[2].padStart(2, '0')}`
  }
  return undefined
}

// ---------- Tool execution ----------

async function runIntent(intent: Intent, ctx: AgentContext): Promise<AgentCard[]> {
  switch (intent.kind) {
    case 'help':
      return [
        {
          kind: 'info',
          text: [
            '我可以帮你处理案件工作台里的常规操作：',
            '· 查看案件 / 时间轴节点 / 证据文件 / 人物',
            '· 搜索全部材料（支持中英文与双语块）',
            '· 人物去重建议与案件内合并',
            '· 生成证据清单（xlsx）与时间轴报告（pdf/html）',
            '',
            '直接输入意图即可，例如："列出最近的时间轴节点"、"搜索补充协议"、"新建节点：签约 2024-01-12"、"导出时间轴报告"。',
            '写操作（新增、修改、删除、导出）都会先进入右侧待确认队列，确认后才执行。',
          ].join('\n'),
        },
      ]

    case 'list_cases': {
      const data = await api.fetchCases(ctx.token)
      return [
        {
          kind: 'info',
          text:
            data.total === 0
              ? '当前没有案件。你可以先在右上角「+ 新案件」创建。'
              : `共 ${data.total} 个案件。`,
        },
      ]
    }

    case 'list_nodes': {
      if (!ctx.caseId) return needCaseCard()
      const data = await api.fetchNodes(ctx.token, ctx.caseId, 1, 200)
      return [
        {
          kind: 'nodes',
          intro: `案件「${ctx.caseName}」共 ${data.total} 个时间轴节点。`,
          nodes: data.nodes,
        },
      ]
    }

    case 'list_files': {
      if (!ctx.caseId) return needCaseCard()
      const data = await api.fetchFiles(ctx.token, ctx.caseId, 1, 50)
      return [
        {
          kind: 'files',
          intro: `案件「${ctx.caseName}」共 ${data.total} 份证据文件。需要上传文件可以点击输入框左侧的「上传证据」。`,
          files: data.files,
        },
      ]
    }

    case 'list_persons': {
      if (!ctx.caseId) return needCaseCard()
      const data = await api.fetchPersons(ctx.token, ctx.caseId, 1, 100)
      const relationships = await api.fetchRelationships(ctx.token, ctx.caseId)
      return [
        {
          kind: 'persons',
          intro: `案件「${ctx.caseName}」共 ${data.total} 个人物。`,
          persons: data.persons.map((p) => ({
            id: p.id,
            name: p.name,
            role_type: p.role_type,
            phone: p.phone,
            organization: p.organization,
          })),
        },
        ...(relationships.relationships.length
          ? [
              {
                kind: 'relationships' as const,
                intro: `${relationships.relationships.length} 条人物关系。`,
                relationships: relationships.relationships,
              },
            ]
          : []),
      ]
    }

    case 'dedupe': {
      if (!ctx.caseId) return needCaseCard()
      const data = await api.fetchDedupeCandidates(ctx.token, ctx.caseId)
      if (!data.groups.length) {
        return [{ kind: 'info', text: '没有发现疑似重复的人物。' }]
      }
      return [
        {
          kind: 'dedupe',
          intro: `发现 ${data.groups.length} 组疑似重复人物，请逐组确认是否合并（合并属于写操作，会进入待确认队列）。`,
          groups: data.groups,
        },
      ]
    }

    case 'search': {
      if (!ctx.caseId) return needCaseCard()
      if (!intent.keyword) {
        return [
          {
            kind: 'info',
            text: '搜索关键词为空。试试：「搜索 补充协议」或「搜索 2024-01」。',
          },
        ]
      }
      const data = await api.search(ctx.token, intent.keyword, undefined, ctx.caseId)
      if (!data.results.length) {
        return [{ kind: 'info', text: `没有找到与「${intent.keyword}」相关的结果。` }]
      }
      return [
        {
          kind: 'search',
          intro: `找到 ${data.total} 条与「${intent.keyword}」相关的结果。`,
          results: data.results,
        },
      ]
    }

    case 'create_node': {
      if (!ctx.caseId) return needCaseCard()
      if (ctx.isReadOnly) {
        return [
          {
            kind: 'error',
            text: '你当前是只读角色（viewer），无法创建时间轴节点。',
          },
        ]
      }
      const title = intent.title
      const date = intent.date
      if (!title || !date) {
        const missing = !title ? '标题' : '日期'
        return [
          {
            kind: 'info',
            text: `创建节点缺少${missing}。请按此格式补充：新建节点「标题」2024-01-12（标题与日期都要有）。`,
          },
        ]
      }
      return [] // handled by caller to build approval
    }

    case 'parse': {
      return [
        {
          kind: 'info',
          text: '请在「证据」视图中选择待解析文件，或先让我列出证据文件，再告诉我解析哪个。',
        },
      ]
    }

    case 'export_evidence_list':
    case 'export_timeline_report':
      if (!ctx.caseId) return needCaseCard()
      if (ctx.isReadOnly) {
        return [
          { kind: 'error', text: '你当前是只读角色（viewer），无法生成导出文件。' },
        ]
      }
      return []

    case 'unknown':
      return [
        {
          kind: 'info',
          text: '我暂时无法理解这条指令。可以试试：列出节点 / 列出证据 / 人物去重 / 搜索 xxx / 导出时间轴报告。',
        },
      ]
  }
}

function needCaseCard(): AgentCard[] {
  return [
    {
      kind: 'info',
      text: '请先选择一个案件（左上角案件切换器），我才能读取该案件的节点、证据与人物。',
    },
  ]
}

// ---------- Public API ----------

export type AgentOutput = {
  text: string
  cards: AgentCard[]
  approvals: ApprovalItem[]
  options?: string[]
}

let approvalSeq = 0

export function nextApprovalId(): string {
  approvalSeq += 1
  return `ap-${Date.now()}-${approvalSeq}`
}

export async function handleUserInput(
  text: string,
  ctx: AgentContext,
  buildApproval: (intent: Intent) => ApprovalItem[],
): Promise<AgentOutput> {
  const intent = parseIntent(text)
  const cards = await runIntent(intent, ctx)

  // Write intents generate approvals instead of results.
  const approvals: ApprovalItem[] = []
  if (
    (intent.kind === 'create_node' ||
      intent.kind === 'export_evidence_list' ||
      intent.kind === 'export_timeline_report') &&
    cards.length === 0
  ) {
    approvals.push(...buildApproval(intent))
  }

  if (approvals.length > 0) {
    const kinds = approvals
      .map((a) => `「${a.title}」`)
      .join('、')
    return {
      text: `已生成 ${approvals.length} 个待确认动作：${kinds}。请在右侧队列中审阅并确认。`,
      cards: [],
      approvals,
    }
  }

  const intro = cards[0] && 'intro' in cards[0] ? cards[0].intro : ''
  return {
    text: intro,
    cards,
    approvals,
  }
}

export function buildApprovalForIntent(
  intent: Intent,
  ctx: AgentContext,
): ApprovalItem[] {
  switch (intent.kind) {
    case 'create_node': {
      const title = intent.title ?? '未命名节点'
      const date = intent.date ?? ''
      return [
        {
          id: nextApprovalId(),
          title: '新建时间轴节点',
          summary: `${title} · ${date}`,
          level: 'write',
          execute: async () => {
            if (!ctx.caseId) throw new Error('no case selected')
            return api.createNode(ctx.token, ctx.caseId, {
              title,
              event_time: date,
            })
          },
        },
      ]
    }
    case 'export_evidence_list':
      return [
        {
          id: nextApprovalId(),
          title: '生成证据清单',
          summary: '导出当前案件全部证据为 xlsx 文件',
          level: 'write',
          execute: async () => {
            if (!ctx.caseId) throw new Error('no case selected')
            return api.exportEvidenceList(ctx.token, ctx.caseId)
          },
        },
      ]
    case 'export_timeline_report':
      return [
        {
          id: nextApprovalId(),
          title: `导出时间轴报告（${intent.format.toUpperCase()}）`,
          summary: '生成时间轴审阅报告',
          level: 'write',
          execute: async () => {
            if (!ctx.caseId) throw new Error('no case selected')
            return api.exportTimelineReport(ctx.token, ctx.caseId, intent.format)
          },
        },
      ]
    default:
      return []
  }
}
