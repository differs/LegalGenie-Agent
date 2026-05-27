import { type FormEvent, useEffect, useMemo, useState } from 'react'
import {
  Activity,
  ArrowRight,
  CheckCircle2,
  Command,
  FileStack,
  LayoutDashboard,
  Layers2,
  LogOut,
  Search,
  Server,
  ShieldCheck,
  UserRound,
} from 'lucide-react'
import clsx from 'clsx'
import {
  Link,
  Navigate,
  NavLink,
  Route,
  Routes,
  useLocation,
  useNavigate,
} from 'react-router-dom'
import {
  activities,
  caseBoard as fallbackCaseBoard,
  commandCenter,
  evidenceByCase,
  metrics,
  pulse,
  timelineByCase,
  workflow,
} from './data'
import {
  ApiError,
  type CaseSummary,
  fetchCases,
  fetchHealth,
  login,
  type LoginPayload,
  type Session,
} from './lib/api'
import { clearSession, loadSession, loadToken, saveSession } from './lib/session'

type HealthState = {
  status: string
  version: string
  database: string
}

type AuthState = {
  session: Session | null
  setSession: (session: Session | null) => void
}

function App() {
  const [session, setSessionState] = useState<Session | null>(() => loadSession())

  const auth = useMemo<AuthState>(
    () => ({
      session,
      setSession: (nextSession) => {
        setSessionState(nextSession)
        if (nextSession) {
          saveSession(nextSession)
        } else {
          clearSession()
        }
      },
    }),
    [session],
  )

  return (
    <div className="min-h-screen bg-[var(--color-ink-950)] text-slate-100">
      <Backdrop />
      <div className="relative z-10 mx-auto flex min-h-screen max-w-7xl flex-col px-4 pb-12 pt-6 sm:px-6 lg:px-8">
        <Header auth={auth} />
        <main className="mt-6 flex-1">
          <Routes>
            <Route path="/" element={<DashboardPage auth={auth} />} />
            <Route path="/cases" element={<CasesPage auth={auth} />} />
            <Route path="/activity" element={<ActivityPage />} />
            <Route path="/system" element={<SystemPage />} />
            <Route path="/login" element={<LoginPage auth={auth} />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
        </main>
      </div>
    </div>
  )
}

function Backdrop() {
  return (
    <>
      <div className="absolute inset-x-0 top-0 -z-0 h-[34rem] bg-[radial-gradient(circle_at_top,rgba(187,148,87,0.22),transparent_45%),linear-gradient(180deg,rgba(7,12,22,0.92),rgba(7,12,22,0))]" />
      <div className="absolute left-[-12rem] top-48 -z-0 h-80 w-80 rounded-full bg-[rgba(28,77,138,0.22)] blur-3xl" />
      <div className="absolute right-[-6rem] top-32 -z-0 h-72 w-72 rounded-full bg-[rgba(186,145,76,0.18)] blur-3xl" />
    </>
  )
}

function Header({ auth }: { auth: AuthState }) {
  const location = useLocation()
  const navigate = useNavigate()

  const navItems = [
    { to: '/', label: 'Dashboard', icon: LayoutDashboard },
    { to: '/cases', label: 'Cases', icon: FileStack },
    { to: '/activity', label: 'Activity', icon: Activity },
    { to: '/system', label: 'System', icon: Server },
  ]

  return (
    <header className="glass-panel flex flex-col gap-4 px-5 py-4 lg:flex-row lg:items-center lg:justify-between">
      <div>
        <p className="eyebrow">Legal Operations Intelligence</p>
        <div className="mt-2 flex items-center gap-3">
          <div className="flex h-11 w-11 items-center justify-center rounded-2xl border border-amber-200/20 bg-amber-300/10 text-amber-200">
            <Command className="h-5 w-5" />
          </div>
          <div>
            <h1 className="font-serif text-xl text-white sm:text-2xl">
              LegalGenie Agent
            </h1>
            <p className="text-sm text-slate-300">
              Open-source legal workspace for disputes, evidence and audit trails
            </p>
          </div>
        </div>
      </div>

      <div className="flex flex-col gap-3 lg:items-end">
        <nav className="flex flex-wrap gap-2">
          {navItems.map((item) => {
            const Icon = item.icon

            return (
              <NavLink
                key={item.to}
                to={item.to}
                className={({ isActive }) =>
                  clsx(
                    'inline-flex items-center gap-2 rounded-full border px-4 py-2 text-sm transition',
                    isActive
                      ? 'border-amber-200/35 bg-amber-300/10 text-amber-100'
                      : 'border-white/12 bg-white/6 text-slate-300 hover:border-white/20 hover:bg-white/10',
                  )
                }
              >
                <Icon className="h-4 w-4" />
                {item.label}
              </NavLink>
            )
          })}
        </nav>

        <div className="flex flex-wrap items-center gap-3">
          {auth.session ? (
            <>
              <div className="inline-flex items-center gap-2 rounded-full border border-emerald-400/20 bg-emerald-400/10 px-4 py-2 text-sm text-emerald-100">
                <UserRound className="h-4 w-4" />
                {auth.session.user.username}
              </div>
              <button
                type="button"
                className="inline-flex items-center gap-2 rounded-full border border-white/12 bg-white/6 px-4 py-2 text-sm text-slate-200 transition hover:border-amber-200/30 hover:bg-white/10"
                onClick={() => auth.setSession(null)}
              >
                <LogOut className="h-4 w-4" />
                Logout
              </button>
            </>
          ) : (
            <button
              type="button"
              className="inline-flex items-center gap-2 rounded-full bg-amber-300 px-4 py-2 text-sm font-semibold text-slate-950 transition hover:bg-amber-200"
              onClick={() => {
                if (location.pathname !== '/login') {
                  navigate('/login')
                }
              }}
            >
              Connect API
              <ArrowRight className="h-4 w-4" />
            </button>
          )}
        </div>
      </div>
    </header>
  )
}

function DashboardPage({ auth }: { auth: AuthState }) {
  const [health, setHealth] = useState<HealthState | null>(null)
  const [healthError, setHealthError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false

    fetchHealth()
      .then((data) => {
        if (!cancelled) {
          setHealth(data)
          setHealthError(null)
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setHealthError(error instanceof Error ? error.message : 'Unable to reach API')
        }
      })

    return () => {
      cancelled = true
    }
  }, [])

  return (
    <div className="space-y-6">
      <section className="grid gap-6 lg:grid-cols-[1.35fr_0.95fr]">
        <div className="glass-panel overflow-hidden px-6 py-6 sm:px-8">
          <p className="eyebrow">Professional Frontend / Vite + Bun + Tailwind</p>
          <div className="mt-4 max-w-3xl">
            <h2 className="font-serif text-4xl leading-tight text-white sm:text-5xl">
              把案件材料、事实链和协作动作
              <span className="text-amber-200"> 收拢到同一张作战桌面</span>
            </h2>
            <p className="mt-5 max-w-2xl text-base leading-7 text-slate-300 sm:text-lg">
              这套开源前端现在已经能连到现有后端 API，支持健康检查、登录和真实案件列表读取；
              同时保留法律团队熟悉的时间轴、证据与审计视角。
            </p>
          </div>

          <div className="mt-8 grid gap-3 sm:grid-cols-3">
            {['案件主视图', '双语证据检索', '审计闭环导出'].map((item) => (
              <div
                key={item}
                className="rounded-2xl border border-white/10 bg-white/5 px-4 py-4 text-sm text-slate-200"
              >
                <CheckCircle2 className="mb-3 h-5 w-5 text-amber-200" />
                {item}
              </div>
            ))}
          </div>
        </div>

        <aside className="glass-panel px-5 py-5">
          <div className="flex items-center justify-between">
            <div>
              <p className="eyebrow">Backend Status</p>
              <h3 className="mt-2 font-serif text-2xl text-white">运行连接状态</h3>
            </div>
            <div
              className={clsx(
                'rounded-full px-3 py-1 text-xs',
                health?.status === 'ok'
                  ? 'border border-emerald-400/20 bg-emerald-400/10 text-emerald-200'
                  : 'border border-amber-400/20 bg-amber-400/10 text-amber-100',
              )}
            >
              {health?.status === 'ok' ? 'API online' : 'Waiting for API'}
            </div>
          </div>

          <div className="mt-5 space-y-3">
            <div className="rounded-2xl border border-white/10 bg-slate-950/40 px-4 py-4">
              <p className="text-sm text-slate-300">API base URL</p>
              <p className="mt-2 text-sm text-slate-400">
                {import.meta.env.VITE_API_BASE_URL ?? 'http://127.0.0.1:8001/api/v1'}
              </p>
            </div>
            <div className="rounded-2xl border border-white/10 bg-slate-950/40 px-4 py-4">
              <p className="text-sm text-slate-300">Health version</p>
              <p className="mt-2 text-2xl font-semibold text-white">
                {health?.version ?? '--'}
              </p>
            </div>
            <div className="rounded-2xl border border-white/10 bg-slate-950/40 px-4 py-4">
              <p className="text-sm text-slate-300">Database</p>
              <p className="mt-2 text-2xl font-semibold text-white">
                {health?.database ?? '--'}
              </p>
              {healthError ? (
                <p className="mt-3 text-sm leading-6 text-rose-200">{healthError}</p>
              ) : null}
            </div>
          </div>
        </aside>
      </section>

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {metrics.map((item) => {
          const Icon = item.icon

          return (
            <article key={item.label} className="glass-panel px-5 py-5">
              <div className="flex items-center justify-between">
                <p className="text-sm text-slate-300">{item.label}</p>
                <div className="rounded-2xl border border-white/10 bg-white/6 p-2.5">
                  <Icon className="h-5 w-5 text-amber-200" />
                </div>
              </div>
              <p className="mt-5 text-4xl font-semibold tracking-tight text-white">
                {item.value}
              </p>
              <p className="mt-3 text-sm leading-6 text-slate-400">{item.detail}</p>
            </article>
          )
        })}
      </section>

      <section className="grid gap-6 xl:grid-cols-[0.95fr_1.45fr]">
        <div className="glass-panel px-5 py-5">
          <div className="flex items-center justify-between">
            <div>
              <p className="eyebrow">Workflow</p>
              <h3 className="mt-2 font-serif text-2xl text-white">标准办案路径</h3>
            </div>
            <Layers2 className="h-5 w-5 text-slate-300" />
          </div>

          <div className="mt-5 space-y-4">
            {workflow.map((item) => {
              const Icon = item.icon

              return (
                <div
                  key={item.title}
                  className="rounded-2xl border border-white/10 bg-white/4 px-4 py-4"
                >
                  <div className="flex items-start gap-4">
                    <div className="rounded-2xl border border-amber-200/15 bg-amber-300/10 p-3">
                      <Icon className="h-5 w-5 text-amber-200" />
                    </div>
                    <div>
                      <p className="text-xs uppercase tracking-[0.22em] text-slate-500">
                        {item.tag}
                      </p>
                      <h4 className="mt-2 text-lg font-semibold text-white">
                        {item.title}
                      </h4>
                      <p className="mt-2 text-sm leading-6 text-slate-400">
                        {item.description}
                      </p>
                    </div>
                  </div>
                </div>
              )
            })}
          </div>
        </div>

        <div className="glass-panel px-5 py-5">
          <div className="flex items-center justify-between">
            <div>
              <p className="eyebrow">API Integration</p>
              <h3 className="mt-2 font-serif text-2xl text-white">下一步入口</h3>
            </div>
            <ShieldCheck className="h-5 w-5 text-amber-200" />
          </div>
          <div className="mt-5 grid gap-4 md:grid-cols-2">
            <Link
              to="/cases"
              className="rounded-3xl border border-white/10 bg-slate-950/55 px-5 py-5 transition hover:border-white/20 hover:bg-white/6"
            >
              <p className="text-sm text-slate-400">真实数据</p>
              <h4 className="mt-2 text-xl font-semibold text-white">案件列表</h4>
              <p className="mt-3 text-sm leading-6 text-slate-400">
                登录后读取 `/cases`，展示状态、创建时间和计数聚合。
              </p>
            </Link>
            <Link
              to={auth.session ? '/system' : '/login'}
              className="rounded-3xl border border-white/10 bg-white/4 px-5 py-5 transition hover:border-white/20 hover:bg-white/6"
            >
              <p className="text-sm text-slate-400">连接入口</p>
              <h4 className="mt-2 text-xl font-semibold text-white">
                {auth.session ? '系统状态' : '连接 API'}
              </h4>
              <p className="mt-3 text-sm leading-6 text-slate-400">
                {auth.session
                  ? '查看会话、基础配置和开源接入说明。'
                  : '用现有账号直接登录后端，不需要另外写 mock server。'}
              </p>
            </Link>
          </div>
        </div>
      </section>
    </div>
  )
}

function CasesPage({ auth }: { auth: AuthState }) {
  const [cases, setCases] = useState<CaseSummary[]>([])
  const [loading, setLoading] = useState(Boolean(auth.session))
  const [error, setError] = useState<string | null>(null)
  const [activeCaseId, setActiveCaseId] = useState<string>(fallbackCaseBoard[0].id)

  useEffect(() => {
    const token = loadToken()
    if (!token) {
      return
    }

    let cancelled = false

    fetchCases(token)
      .then((data) => {
        if (!cancelled) {
          setCases(data.cases)
          setError(null)
          if (data.cases[0]) {
            setActiveCaseId(data.cases[0].id)
          }
        }
      })
      .catch((apiError: unknown) => {
        if (!cancelled) {
          if (apiError instanceof ApiError && apiError.status === 401) {
            auth.setSession(null)
          }
          setError(apiError instanceof Error ? apiError.message : 'Failed to load cases')
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false)
        }
      })

    return () => {
      cancelled = true
    }
  }, [auth])

  const hasLiveCases = Boolean(auth.session) && cases.length > 0
  const selectedFallback =
    fallbackCaseBoard.find((item) => item.id === activeCaseId) ?? fallbackCaseBoard[0]
  const selectedCase = cases.find((item) => item.id === activeCaseId)

  return (
    <div className="grid gap-6 xl:grid-cols-[0.9fr_1.1fr]">
      <div className="glass-panel px-5 py-5">
        <div className="flex items-center justify-between">
          <div>
            <p className="eyebrow">Matter Board</p>
            <h2 className="mt-2 font-serif text-2xl text-white">案件列表</h2>
            <p className="mt-2 text-sm leading-6 text-slate-400">
              {auth.session
                ? '优先展示后端实时案件数据；若当前账号还没有案件，则保留演示版布局。'
                : '未登录时显示演示数据。登录后会自动切换到真实后端。'}
            </p>
          </div>
          <div className="rounded-full border border-white/10 bg-white/5 px-3 py-1 text-xs text-slate-300">
            {hasLiveCases ? 'Live API' : 'Demo'}
          </div>
        </div>

        {loading ? <StateBanner label="Loading cases from API..." /> : null}
        {error ? <StateBanner label={error} tone="error" /> : null}

        <div className="mt-5 space-y-3">
          {(hasLiveCases ? cases : fallbackCaseBoard).map((item) => {
            const isLive = 'member_count' in item
            const caseId = item.id

            return (
              <button
                key={caseId}
                type="button"
                onClick={() => setActiveCaseId(caseId)}
                className={clsx(
                  'w-full rounded-3xl border px-4 py-4 text-left transition',
                  caseId === activeCaseId
                    ? 'border-amber-200/35 bg-amber-300/10 shadow-[0_0_0_1px_rgba(255,224,171,0.08)]'
                    : 'border-white/10 bg-white/4 hover:border-white/20 hover:bg-white/6',
                )}
              >
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <p className="text-xs uppercase tracking-[0.22em] text-slate-500">
                      {item.id}
                    </p>
                    <h3 className="mt-2 text-lg font-semibold text-white">{item.name}</h3>
                  </div>
                  <span className="rounded-full bg-white/7 px-3 py-1 text-xs text-slate-200">
                    {isLive ? item.status : item.risk}
                  </span>
                </div>
                <div className="mt-4 flex flex-wrap gap-2 text-xs text-slate-300">
                  {isLive ? (
                    <>
                      <span className="rounded-full bg-white/7 px-3 py-1">
                        members {item.member_count}
                      </span>
                      <span className="rounded-full bg-white/7 px-3 py-1">
                        evidence {item.evidence_count}
                      </span>
                      <span className="rounded-full bg-white/7 px-3 py-1">
                        timeline {item.node_count}
                      </span>
                    </>
                  ) : (
                    <>
                      <span className="rounded-full bg-white/7 px-3 py-1">{item.stage}</span>
                      <span className="rounded-full bg-white/7 px-3 py-1">
                        {item.jurisdiction}
                      </span>
                      <span className="rounded-full bg-white/7 px-3 py-1">
                        Lead: {item.lead}
                      </span>
                    </>
                  )}
                </div>
              </button>
            )
          })}
        </div>
      </div>

      <div className="grid gap-6">
        <div className="glass-panel px-5 py-5">
          <p className="eyebrow">Selected Case</p>
          <h2 className="mt-2 font-serif text-2xl text-white">
            {selectedCase?.name ?? selectedFallback.name}
          </h2>
          <p className="mt-4 text-sm leading-7 text-slate-300">
            {selectedCase?.description ??
              selectedFallback.summary ??
              'No case description is currently available from the backend.'}
          </p>
          <div className="mt-5 grid grid-cols-2 gap-3 text-sm">
            <div className="rounded-2xl border border-white/10 bg-white/5 px-4 py-3">
              <p className="text-slate-400">Evidence</p>
              <p className="mt-2 text-2xl font-semibold text-white">
                {selectedCase?.evidence_count ?? selectedFallback.evidenceCount}
              </p>
            </div>
            <div className="rounded-2xl border border-white/10 bg-white/5 px-4 py-3">
              <p className="text-slate-400">Timeline</p>
              <p className="mt-2 text-2xl font-semibold text-white">
                {selectedCase?.node_count ?? selectedFallback.timelineCount}
              </p>
            </div>
          </div>
        </div>

        <div className="grid gap-6 lg:grid-cols-2">
          <div className="glass-panel px-5 py-5">
            <div className="flex items-center justify-between">
              <div>
                <p className="eyebrow">Evidence Layer</p>
                <h3 className="mt-2 font-serif text-xl text-white">演示证据视图</h3>
              </div>
              <Search className="h-5 w-5 text-amber-200" />
            </div>
            <div className="mt-5 space-y-3">
              {(evidenceByCase[selectedFallback.id] ?? []).map((item) => (
                <article
                  key={item.title}
                  className="rounded-3xl border border-white/10 bg-slate-950/45 px-5 py-4"
                >
                  <h4 className="text-lg font-semibold text-white">{item.title}</h4>
                  <p className="mt-2 text-sm leading-6 text-slate-400">
                    {item.source} · {item.language}
                  </p>
                </article>
              ))}
            </div>
          </div>

          <div className="glass-panel px-5 py-5">
            <p className="eyebrow">Timeline Narrative</p>
            <h3 className="mt-2 font-serif text-xl text-white">演示事实链</h3>
            <div className="mt-6 space-y-5">
              {(timelineByCase[selectedFallback.id] ?? []).map((item) => (
                <div key={`${item.date}-${item.title}`} className="flex gap-4">
                  <div className="flex w-20 shrink-0 flex-col items-center">
                    <div className="h-3 w-3 rounded-full bg-amber-200" />
                    <div className="mt-2 h-full w-px bg-white/10" />
                  </div>
                  <div className="pb-5">
                    <p className="text-xs uppercase tracking-[0.2em] text-slate-500">
                      {item.date}
                    </p>
                    <h4 className="mt-2 text-lg font-semibold text-white">
                      {item.title}
                    </h4>
                    <p className="mt-2 text-sm leading-6 text-slate-400">
                      {item.detail}
                    </p>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

function ActivityPage() {
  return (
    <div className="grid gap-6 lg:grid-cols-[0.9fr_1.1fr]">
      <div className="glass-panel px-5 py-5">
        <p className="eyebrow">Workspace Services</p>
        <h2 className="mt-2 font-serif text-2xl text-white">能力模块</h2>
        <div className="mt-5 grid gap-3">
          {commandCenter.map((item) => {
            const Icon = item.icon

            return (
              <article
                key={item.label}
                className="rounded-3xl border border-white/10 bg-white/4 px-4 py-4"
              >
                <div className="flex items-start gap-4">
                  <div className="rounded-2xl border border-white/10 bg-slate-950/55 p-3">
                    <Icon className="h-5 w-5 text-amber-200" />
                  </div>
                  <div>
                    <h3 className="text-lg font-semibold text-white">{item.label}</h3>
                    <p className="mt-2 text-sm leading-6 text-slate-400">
                      {item.description}
                    </p>
                  </div>
                </div>
              </article>
            )
          })}
        </div>
      </div>

      <div className="glass-panel px-5 py-5">
        <div className="flex items-center justify-between">
          <div>
            <p className="eyebrow">Activity Stream</p>
            <h2 className="mt-2 font-serif text-2xl text-white">最近动作</h2>
          </div>
          <span className="rounded-full border border-white/10 bg-white/5 px-3 py-1 text-xs text-slate-300">
            Audit Ready
          </span>
        </div>

        <div className="mt-5 space-y-3">
          {activities.map((item) => (
            <article
              key={`${item.actor}-${item.time}`}
              className="rounded-3xl border border-white/10 bg-slate-950/45 px-5 py-4"
            >
              <div className="flex items-center justify-between gap-4">
                <div>
                  <p className="text-sm text-slate-400">{item.actor}</p>
                  <p className="mt-2 text-base leading-7 text-white">{item.action}</p>
                </div>
                <span className="text-xs uppercase tracking-[0.2em] text-slate-500">
                  {item.time}
                </span>
              </div>
            </article>
          ))}
        </div>
      </div>
    </div>
  )
}

function SystemPage() {
  return (
    <div className="grid gap-6 lg:grid-cols-[1.05fr_0.95fr]">
      <div className="glass-panel px-5 py-5">
        <p className="eyebrow">Open Source Setup</p>
        <h2 className="mt-2 font-serif text-2xl text-white">接入说明</h2>
        <div className="mt-5 space-y-3 text-sm leading-7 text-slate-300">
          <p>
            `web/` 默认通过 `VITE_API_BASE_URL` 连接后端；未设置时回退到
            `http://127.0.0.1:8001/api/v1`。
          </p>
          <p>
            已接通的链路包括：`GET /health`、`POST /auth/login`、`GET /cases`。
          </p>
          <p>
            你可以继续在这个前端上扩展文件上传、人物图谱、时间轴编辑和导出中心。
          </p>
        </div>
      </div>

      <div className="space-y-6">
        <div className="glass-panel px-5 py-5">
          <p className="eyebrow">Operational Pulse</p>
          <h2 className="mt-2 font-serif text-2xl text-white">运行脉搏</h2>
          <div className="mt-5 space-y-3">
            {pulse.map((item) => {
              const Icon = item.icon

              return (
                <div
                  key={item.title}
                  className="rounded-2xl border border-white/10 bg-slate-950/40 px-4 py-4"
                >
                  <div className="flex items-start justify-between gap-4">
                    <div>
                      <p className="text-sm text-slate-300">{item.title}</p>
                      <p className="mt-2 text-3xl font-semibold text-white">{item.value}</p>
                    </div>
                    <div className="rounded-2xl border border-white/10 bg-white/6 p-3">
                      <Icon className="h-5 w-5 text-amber-200" />
                    </div>
                  </div>
                  <p className="mt-3 text-sm leading-6 text-slate-400">{item.detail}</p>
                </div>
              )
            })}
          </div>
        </div>
      </div>
    </div>
  )
}

function LoginPage({ auth }: { auth: AuthState }) {
  const navigate = useNavigate()
  const [form, setForm] = useState<LoginPayload>({
    username: '',
    password: '',
    remember_me: true,
  })
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (auth.session) {
      navigate('/cases', { replace: true })
    }
  }, [auth.session, navigate])

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setSubmitting(true)
    setError(null)

    try {
      const session = await login(form)
      auth.setSession(session)
      navigate('/cases', { replace: true })
    } catch (loginError: unknown) {
      setError(loginError instanceof Error ? loginError.message : 'Login failed')
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="mx-auto max-w-xl">
      <div className="glass-panel px-6 py-6 sm:px-8">
        <p className="eyebrow">Backend Authentication</p>
        <h2 className="mt-2 font-serif text-3xl text-white">连接现有 API</h2>
        <p className="mt-4 text-sm leading-7 text-slate-300">
          使用后端已有账号登录，前端会保存 access token 并调用真实案件接口。
        </p>

        <form className="mt-6 space-y-4" onSubmit={handleSubmit}>
          <label className="block">
            <span className="mb-2 block text-sm text-slate-300">Username or Email</span>
            <input
              required
              value={form.username}
              onChange={(event) =>
                setForm((current) => ({ ...current, username: event.target.value }))
              }
              className="w-full rounded-2xl border border-white/10 bg-slate-950/60 px-4 py-3 text-white outline-none transition focus:border-amber-200/35"
            />
          </label>
          <label className="block">
            <span className="mb-2 block text-sm text-slate-300">Password</span>
            <input
              required
              type="password"
              value={form.password}
              onChange={(event) =>
                setForm((current) => ({ ...current, password: event.target.value }))
              }
              className="w-full rounded-2xl border border-white/10 bg-slate-950/60 px-4 py-3 text-white outline-none transition focus:border-amber-200/35"
            />
          </label>
          <label className="flex items-center gap-3 text-sm text-slate-300">
            <input
              type="checkbox"
              checked={form.remember_me}
              onChange={(event) =>
                setForm((current) => ({
                  ...current,
                  remember_me: event.target.checked,
                }))
              }
            />
            remember me
          </label>
          {error ? <StateBanner label={error} tone="error" /> : null}
          <button
            type="submit"
            disabled={submitting}
            className="inline-flex items-center gap-2 rounded-full bg-amber-300 px-5 py-3 text-sm font-semibold text-slate-950 transition hover:bg-amber-200 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {submitting ? 'Connecting...' : 'Login'}
            <ArrowRight className="h-4 w-4" />
          </button>
        </form>
      </div>
    </div>
  )
}

function StateBanner({
  label,
  tone = 'neutral',
}: {
  label: string
  tone?: 'neutral' | 'error'
}) {
  return (
    <div
      className={clsx(
        'mt-4 rounded-2xl border px-4 py-3 text-sm',
        tone === 'error'
          ? 'border-rose-300/20 bg-rose-400/10 text-rose-100'
          : 'border-white/10 bg-white/5 text-slate-300',
      )}
    >
      {label}
    </div>
  )
}

export default App
