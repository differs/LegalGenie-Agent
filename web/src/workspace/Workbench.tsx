import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import { useNavigate } from 'react-router-dom'
import { Bot, LogOut, Scale, UserRound } from 'lucide-react'
import { useAuth } from '../App'
import { fetchCases, fetchMyRole, logout, uploadFile as apiUploadFile } from '../lib/api'
import { useToasts, type Toast } from '../lib/hooks'
import type { ApprovalItem, ChatMessage } from '../lib/agent'
import type {
  CaseRole,
  CaseSummary,
  CasePerson,
  EvidenceFileSummary,
  TimelineNode,
  SearchResult,
} from '../lib/types'
import { LeftRail, type WorkspaceView } from './LeftRail'
import { ContextPanel } from './ContextPanel'
import { ChatView } from './ChatView'
import { TimelineView } from './TimelineView'
import { EvidenceView } from './EvidenceView'
import { PersonsView } from './PersonsView'
import { SearchView } from './SearchView'
import { ExportsView } from './ExportsView'
import { LogsView } from './LogsView'

export type Selection =
  | { kind: 'node'; node: TimelineNode }
  | { kind: 'file'; file: EvidenceFileSummary }
  | { kind: 'person'; person: CasePerson }
  | { kind: 'search'; hit: SearchResult }

type WorkspaceContextValue = {
  token: string
  cases: CaseSummary[]
  caseId: string | null
  caseName: string | null
  role: CaseRole | null
  isReadOnly: boolean
  dataTick: number
  bump: () => void
  selectCase: (id: string) => void
  refreshCases: () => Promise<CaseSummary[]>
  selection: Selection | null
  setSelection: (s: Selection | null) => void
  openView: (v: WorkspaceView) => void
  focusNodeId: string | null
  setFocusNodeId: (id: string | null) => void
  addApprovals: (items: ApprovalItem[]) => void
  onNeedCase: () => void
  onUploadRequested: () => void
  uploadFile: (file: File) => Promise<void>
}

const WorkspaceContext = createContext<WorkspaceContextValue | null>(null)

export function useWorkspace(): WorkspaceContextValue {
  const ctx = useContext(WorkspaceContext)
  if (!ctx) throw new Error('useWorkspace must be used within Workbench')
  return ctx
}

const SESSION_CASE_KEY = 'legalgenie-agent/last-case'

export function Workbench() {
  const { auth } = useAuth()
  const navigate = useNavigate()

  useEffect(() => {
    if (auth.status === 'signed-out') {
      navigate('/login', { replace: true })
    }
  }, [auth.status, navigate])

  if (auth.status !== 'signed-in') {
    return null
  }

  return <WorkbenchInner token={auth.session.accessToken} />
}

function WorkbenchInner({ token }: { token: string }) {
  const { auth, signOut } = useAuth()
  const { toasts, ok, error } = useToasts()

  const [cases, setCases] = useState<CaseSummary[]>([])
  const [caseId, setCaseId] = useState<string | null>(null)
  const [role, setRole] = useState<CaseRole | null>(null)
  const [view, setView] = useState<WorkspaceView>('chat')
  const [selection, setSelection] = useState<Selection | null>(null)
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [approvals, setApprovals] = useState<ApprovalItem[]>([])
  const [dataTick, setDataTick] = useState(0)
  const [focusNodeId, setFocusNodeId] = useState<string | null>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const bump = useCallback(() => setDataTick((t) => t + 1), [])

  const refreshCases = useCallback(async (): Promise<CaseSummary[]> => {
    try {
      const data = await fetchCases(token)
      setCases(data.cases)
      return data.cases
    } catch {
      return cases
    }
  }, [token, cases])

  // ---------- Case loading ----------
  useEffect(() => {
    let alive = true
    fetchCases(token)
      .then((data) => {
        if (!alive) return
        setCases(data.cases)
        const saved = window.localStorage.getItem(SESSION_CASE_KEY)
        const initial =
          (saved && data.cases.find((c) => c.id === saved)?.id) || data.cases[0]?.id || null
        setCaseId(initial)
      })
      .catch((e) => {
        if (!alive) return
        if (e instanceof Error && 'status' in e && (e as { status: number }).status === 401) {
          signOut()
        }
      })
    return () => {
      alive = false
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token])

  // ---------- Role per case ----------
  useEffect(() => {
    if (!caseId || !token) return
    let alive = true
    window.localStorage.setItem(SESSION_CASE_KEY, caseId)
    fetchMyRole(token, caseId)
      .then((r) => alive && setRole(r.role_in_case))
      .catch(() => alive && setRole(null))
    return () => {
      alive = false
    }
  }, [caseId, token])

  // ---------- Per-case chat history (session-scoped) ----------
  useEffect(() => {
    if (!caseId) {
      setMessages([])
      setApprovals([])
      return
    }
    const key = `legalgenie-agent/chat/${caseId}`
    try {
      const raw = window.sessionStorage.getItem(key)
      const parsed = raw ? (JSON.parse(raw) as ChatMessage[]) : []
      // Approvals are not serializable (functions); restore text trail only.
      setMessages(parsed.map((m) => ({ ...m, approvals: [] })))
    } catch {
      setMessages([])
    }
    setApprovals([])
    setSelection(null)
  }, [caseId])

  const persistMessages = useCallback(
    (next: ChatMessage[]) => {
      setMessages(next)
      if (caseId) {
        window.sessionStorage.setItem(`legalgenie-agent/chat/${caseId}`, JSON.stringify(next))
      }
    },
    [caseId],
  )

  const appendMessage = useCallback(
    (msg: ChatMessage) => {
      persistMessages([...messages, msg])
    },
    [messages, persistMessages],
  )

  const caseName = caseId ? cases.find((c) => c.id === caseId)?.name ?? null : null

  const onNeedCase = useCallback(() => {
    ok('请先在左上角选择一个案件')
  }, [ok])

  const onUploadRequested = useCallback(() => {
    fileInputRef.current?.click()
  }, [])

  const uploadFile = useCallback(
    async (file: File) => {
      if (!caseId) {
        error('请先选择案件')
        return
      }
      const created = await apiUploadFile(token, caseId, file)
      bump()
      appendMessage({
        role: 'agent',
        text: `已上传证据文件「${created.original_name}」（${(file.size / 1024).toFixed(1)} KB）。可稍后在证据视图或对话中触发解析。`,
        cards: [
          {
            kind: 'files',
            intro: '最新上传：',
            files: [created],
          },
        ],
        approvals: [],
        at: Date.now(),
      })
      ok(`已上传 ${created.original_name}`)
    },
    [caseId, token, bump, appendMessage, error, ok],
  )

  const handleFileChosen = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0]
      e.target.value = ''
      if (file) void uploadFile(file)
    },
    [uploadFile],
  )

  const handleLogout = useCallback(async () => {
    try {
      if (token) await logout(token)
    } catch {
      // ignore network errors on logout
    }
    signOut()
  }, [token, signOut])

  const ctxValue = useMemo<WorkspaceContextValue>(
    () => ({
      token,
      cases,
      caseId,
      caseName,
      role,
      isReadOnly: role === 'viewer',
      dataTick,
      bump,
      selectCase: (id) => {
        setCaseId(id)
        setView('chat')
      },
      refreshCases,
      selection,
      setSelection,
      openView: setView,
      focusNodeId,
      setFocusNodeId,
      addApprovals: (items) => setApprovals((prev) => [...prev, ...items]),
      onNeedCase,
      onUploadRequested,
      uploadFile,
    }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [token, cases, caseId, caseName, role, dataTick, selection, focusNodeId],
  )

  // ---------- Approval queue actions ----------
  const confirmApproval = useCallback(
    async (a: ApprovalItem) => {
      try {
        await a.execute()
        setApprovals((prev) => prev.filter((p) => p.id !== a.id))
        bump()
        appendMessage({
          role: 'agent',
          text: `已执行「${a.title}」— ${a.summary}`,
          cards: [{ kind: 'info', text: `已执行：${a.title}` }],
          approvals: [],
          at: Date.now(),
        })
        ok(`已执行：${a.title}`)
      } catch (e) {
        setApprovals((prev) => prev.filter((p) => p.id !== a.id))
        error(e instanceof Error ? `执行失败：${e.message}` : '执行失败')
      }
    },
    [appendMessage, bump, error, ok],
  )

  const rejectApproval = useCallback(
    (a: ApprovalItem) => {
      a.onReject?.()
      setApprovals((prev) => prev.filter((p) => p.id !== a.id))
      appendMessage({
        role: 'agent',
        text: `已取消「${a.title}」，未做任何写入。`,
        cards: [],
        approvals: [],
        at: Date.now(),
      })
    },
    [appendMessage],
  )

  // ---------- Render ----------
  return (
    <WorkspaceContext.Provider value={ctxValue}>
      <div className="flex h-screen flex-col overflow-hidden bg-stone-950 text-stone-100">
        {/* Topbar */}
        <header className="flex h-14 shrink-0 items-center gap-3 border-b border-white/10 bg-stone-950/90 px-4">
          <a href="#/" className="flex items-center gap-2.5">
            <div className="flex h-8 w-8 items-center justify-center rounded-xl border border-amber-200/20 bg-amber-300/10 text-amber-200">
              <Scale className="h-4 w-4" />
            </div>
            <div className="leading-tight">
              <p className="text-[0.58rem] font-semibold uppercase tracking-[0.24em] text-stone-500">
                Legal AI Workspace
              </p>
              <p className="font-serif text-sm text-white">LegalGenie Agent</p>
            </div>
          </a>

          <div className="mx-2 h-6 w-px bg-white/10" />

          {/* Case switcher */}
          <label className="flex items-center gap-2 text-xs text-stone-500">
            案件
            <select
              value={caseId ?? ''}
              onChange={(e) => ctxValue.selectCase(e.target.value)}
              className="max-w-[16rem] rounded-lg border border-white/10 bg-stone-900 px-3 py-1.5 text-sm text-stone-200 outline-none transition focus:border-amber-300/50"
            >
              {!caseId && <option value="">未选择</option>}
              {cases.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
          </label>

          {role && (
            <span
              className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-medium ${
                role === 'owner'
                  ? 'border-amber-300/30 bg-amber-300/10 text-amber-100'
                  : role === 'member'
                    ? 'border-emerald-400/25 bg-emerald-400/8 text-emerald-100'
                    : 'border-stone-500/30 bg-stone-500/10 text-stone-300'
              }`}
            >
              <Bot className="h-3 w-3" />
              {role === 'owner' ? 'Owner 所有者' : role === 'member' ? 'Member 成员' : 'Viewer 只读'}
            </span>
          )}

          <div className="flex-1" />

          {auth.status === 'signed-in' && (
            <span className="flex items-center gap-1.5 text-xs text-stone-400">
              <UserRound className="h-3.5 w-3.5" />
              {auth.session.user.real_name || auth.session.user.username}
            </span>
          )}
          <button
            onClick={handleLogout}
            className="flex items-center gap-1.5 rounded-lg border border-white/10 px-3 py-1.5 text-xs text-stone-400 transition hover:border-red-400/30 hover:text-red-200"
            title="退出登录"
          >
            <LogOut className="h-3.5 w-3.5" />
            退出
          </button>
        </header>

        <div className="flex min-h-0 flex-1">
          {/* Left rail */}
          <LeftRail view={view} onView={setView} approvalCount={approvals.length} />

          {/* Main view */}
          <main className="min-w-0 flex-1 overflow-hidden">
            {view === 'chat' && (
              <ChatView
                messages={messages}
                appendMessage={appendMessage}
                approvals={approvals}
                setApprovals={setApprovals}
              />
            )}
            {view === 'timeline' && <TimelineView />}
            {view === 'evidence' && <EvidenceView />}
            {view === 'persons' && <PersonsView />}
            {view === 'search' && <SearchView />}
            {view === 'exports' && <ExportsView />}
            {view === 'logs' && <LogsView />}
          </main>

          {/* Right context panel */}
          <ContextPanel
            approvals={approvals}
            onConfirm={confirmApproval}
            onReject={rejectApproval}
          />
        </div>

        <input ref={fileInputRef} type="file" className="hidden" onChange={handleFileChosen} />

        {/* Toasts */}
        <ToastStack toasts={toasts} />
      </div>
    </WorkspaceContext.Provider>
  )
}

function ToastStack({ toasts }: { toasts: Toast[] }) {
  return (
    <div className="pointer-events-none fixed bottom-5 right-5 z-50 flex w-80 flex-col gap-2">
      {toasts.map((t) => (
        <div
          key={t.id}
          className={`pointer-events-auto rounded-xl border px-4 py-3 text-sm shadow-xl backdrop-blur ${
            t.kind === 'ok'
              ? 'border-emerald-400/25 bg-emerald-950/90 text-emerald-100'
              : t.kind === 'error'
                ? 'border-red-400/25 bg-red-950/90 text-red-100'
                : 'border-amber-300/25 bg-amber-950/90 text-amber-100'
          }`}
        >
          {t.text}
        </div>
      ))}
    </div>
  )
}
