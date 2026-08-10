import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Scale, ShieldCheck, Loader2, AlertCircle } from 'lucide-react'
import * as api from '../lib/api'
import { useAuth } from '../App'

type Mode = 'login' | 'register'

export function AuthPage() {
  const { auth, signIn } = useAuth()
  const navigate = useNavigate()

  const [mode, setMode] = useState<Mode>('login')
  const [username, setUsername] = useState('')
  const [email, setEmail] = useState('')
  const [realName, setRealName] = useState('')
  const [password, setPassword] = useState('')
  const [remember, setRemember] = useState(true)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [serverOk, setServerOk] = useState<boolean | null>(null)

  useEffect(() => {
    api
      .fetchHealth()
      .then(() => setServerOk(true))
      .catch(() => setServerOk(false))
  }, [])

  useEffect(() => {
    if (auth.status === 'signed-in') {
      navigate('/', { replace: true })
    }
  }, [auth.status, navigate])

  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)

    if (!username.trim() || !password) {
      setError('请输入用户名与密码')
      return
    }
    if (mode === 'register' && !email.trim()) {
      setError('注册需要填写邮箱')
      return
    }

    setBusy(true)
    try {
      const session =
        mode === 'login'
          ? await api.login({ username: username.trim(), password, remember_me: remember })
          : await api.register({
              username: username.trim(),
              email: email.trim(),
              password,
              real_name: realName.trim() || undefined,
            })
      signIn(session)
    } catch (e) {
      setError(e instanceof Error ? e.message : '请求失败，请检查后端服务是否启动')
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="relative flex min-h-screen items-center justify-center overflow-hidden bg-stone-950 px-4 text-stone-100">
      <div className="pointer-events-none absolute inset-x-0 top-0 h-[28rem] bg-[radial-gradient(circle_at_top,rgba(208,170,99,0.16),transparent_55%)]" />

      <div className="relative z-10 w-full max-w-md">
        <div className="mb-8 flex flex-col items-center gap-3 text-center">
          <div className="flex h-14 w-14 items-center justify-center rounded-2xl border border-amber-200/20 bg-amber-300/10 text-amber-200">
            <Scale className="h-7 w-7" />
          </div>
          <div>
            <p className="text-[0.64rem] font-semibold uppercase tracking-[0.3em] text-stone-500">
              Legal AI Workspace
            </p>
            <h1 className="mt-1 font-serif text-3xl text-white">LegalGenie Agent</h1>
          </div>
          {serverOk === true && (
            <span className="inline-flex items-center gap-1.5 rounded-full border border-emerald-400/20 bg-emerald-400/8 px-3 py-1 text-xs text-emerald-200">
              <ShieldCheck className="h-3.5 w-3.5" />
              后端服务在线
            </span>
          )}
          {serverOk === false && (
            <span className="inline-flex items-center gap-1.5 rounded-full border border-amber-300/20 bg-amber-300/8 px-3 py-1 text-xs text-amber-200">
              <AlertCircle className="h-3.5 w-3.5" />
              后端未连接（默认 http://127.0.0.1:8001/api/v1）
            </span>
          )}
        </div>

        <div className="rounded-[24px] border border-white/10 bg-white/5 p-7 shadow-2xl shadow-black/40 backdrop-blur">
          <div className="mb-6 grid grid-cols-2 gap-1 rounded-full border border-white/10 bg-stone-950/60 p-1">
            {(
              [
                ['login', '登录'],
                ['register', '注册'],
              ] as const
            ).map(([m, label]) => (
              <button
                key={m}
                type="button"
                onClick={() => {
                  setMode(m)
                  setError(null)
                }}
                className={`rounded-full py-2 text-sm font-medium transition ${
                  mode === m
                    ? 'bg-amber-300 text-stone-950'
                    : 'text-stone-400 hover:text-stone-200'
                }`}
              >
                {label}
              </button>
            ))}
          </div>

          <form onSubmit={submit} className="flex flex-col gap-4">
            {mode === 'register' && (
              <input
                value={realName}
                onChange={(e) => setRealName(e.target.value)}
                placeholder="姓名（可选）"
                className="rounded-xl border border-white/10 bg-stone-950/60 px-4 py-3 text-sm outline-none transition placeholder:text-stone-600 focus:border-amber-300/50"
              />
            )}
            <input
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="用户名"
              autoComplete="username"
              className="rounded-xl border border-white/10 bg-stone-950/60 px-4 py-3 text-sm outline-none transition placeholder:text-stone-600 focus:border-amber-300/50"
            />
            {mode === 'register' && (
              <input
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="邮箱"
                type="email"
                autoComplete="email"
                className="rounded-xl border border-white/10 bg-stone-950/60 px-4 py-3 text-sm outline-none transition placeholder:text-stone-600 focus:border-amber-300/50"
              />
            )}
            <input
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="密码"
              type="password"
              autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
              className="rounded-xl border border-white/10 bg-stone-950/60 px-4 py-3 text-sm outline-none transition placeholder:text-stone-600 focus:border-amber-300/50"
            />

            {mode === 'login' && (
              <label className="flex cursor-pointer items-center gap-2 text-sm text-stone-400">
                <input
                  type="checkbox"
                  checked={remember}
                  onChange={(e) => setRemember(e.target.checked)}
                  className="h-4 w-4 rounded border-white/20 bg-stone-900 accent-amber-300"
                />
                记住我（保持登录 7 天）
              </label>
            )}

            {error && (
              <div className="rounded-xl border border-red-400/20 bg-red-400/8 px-4 py-3 text-sm text-red-200">
                {error}
              </div>
            )}

            <button
              type="submit"
              disabled={busy}
              className="mt-1 flex items-center justify-center gap-2 rounded-xl bg-amber-300 py-3 text-sm font-semibold text-stone-950 transition hover:bg-amber-200 disabled:cursor-not-allowed disabled:opacity-60"
            >
              {busy && <Loader2 className="h-4 w-4 animate-spin" />}
              {mode === 'login' ? '登录工作台' : '创建账号'}
            </button>
          </form>

          <p className="mt-5 text-center text-xs leading-5 text-stone-500">
            自托管部署 · 案件数据仅存于你自己的服务器
            <br />
            演示环境可在后端启动后直接注册新账号
          </p>
        </div>
      </div>
    </div>
  )
}
