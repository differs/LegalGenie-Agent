import { useCallback, useEffect, useRef, useState } from 'react'

// Minimal async-state helper: run a promise and track loading/error/value.
export function useAsync<T>(fn: () => Promise<T>, deps: unknown[]) {
  const [value, setValue] = useState<T | null>(null)
  const [error, setError] = useState<Error | null>(null)
  const [loading, setLoading] = useState(false)
  const [tick, setTick] = useState(0)

  const fnRef = useRef(fn)
  useEffect(() => {
    fnRef.current = fn
  }, [fn])

  const run = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const v = await fnRef.current()
      setValue(v)
    } catch (e) {
      setError(e instanceof Error ? e : new Error(String(e)))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void run()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, tick])

  const refresh = useCallback(() => setTick((t) => t + 1), [])

  return { value, error, loading, refresh, setValue }
}

// Poll a refresh function while `active` is true.
export function usePolling(refresh: () => void, active: boolean, intervalMs = 3000) {
  useEffect(() => {
    if (!active) return
    const timer = window.setInterval(refresh, intervalMs)
    return () => window.clearInterval(timer)
  }, [refresh, active, intervalMs])
}

// ---------- Toasts ----------

export type Toast = {
  id: number
  kind: 'ok' | 'error' | 'info'
  text: string
}

let toastSeq = 0

export function useToasts() {
  const [toasts, setToasts] = useState<Toast[]>([])

  const push = useCallback((kind: Toast['kind'], text: string) => {
    toastSeq += 1
    const id = toastSeq
    setToasts((prev) => [...prev, { id, kind, text }])
    window.setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id))
    }, 4200)
  }, [])

  const ok = useCallback((text: string) => push('ok', text), [push])
  const error = useCallback((text: string) => push('error', text), [push])
  const info = useCallback((text: string) => push('info', text), [push])

  return { toasts, ok, error, info, push }
}

// Formatting helpers shared across views.
export function formatBytes(bytes: number): string {
  if (!bytes) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB']
  const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)))
  return `${(bytes / 1024 ** i).toFixed(i === 0 ? 0 : 1)} ${units[i]}`
}

export function formatDate(iso: string): string {
  return iso.slice(0, 10)
}

export function formatDateTime(iso: string): string {
  return iso.replace('T', ' ').slice(0, 16)
}

export function parseStatusLabel(status: string): { label: string; tone: 'ok' | 'run' | 'warn' } {
  switch (status) {
    case 'done':
      return { label: '已解析', tone: 'ok' }
    case 'processing':
      return { label: '解析中', tone: 'run' }
    case 'pending':
      return { label: '待解析', tone: 'warn' }
    case 'failed':
      return { label: '解析失败', tone: 'warn' }
    default:
      return { label: status, tone: 'warn' }
  }
}

export function translationStatusLabel(status: string): { label: string; tone: 'ok' | 'run' | 'warn' } {
  switch (status) {
    case 'done':
      return { label: '已翻译', tone: 'ok' }
    case 'processing':
      return { label: '翻译中', tone: 'run' }
    case 'pending':
      return { label: '待翻译', tone: 'warn' }
    case 'failed':
      return { label: '翻译失败', tone: 'warn' }
    case 'disabled':
      return { label: '未启用翻译', tone: 'warn' }
    default:
      return { label: status, tone: 'warn' }
  }
}
