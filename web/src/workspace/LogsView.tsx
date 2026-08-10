import { Loader2 } from 'lucide-react'
import { useWorkspace } from './Workbench'
import { fetchLogs } from '../lib/api'
import { formatDateTime, useAsync } from '../lib/hooks'
import type { OperationLog } from '../lib/types'

export function LogsView() {
  const { token, caseId, dataTick } = useWorkspace()
  const { value, loading } = useAsync(
    () => fetchLogs(token, caseId ?? undefined, 1, 100),
    [token, caseId, dataTick],
  )

  const items = value?.logs ?? []

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-2 border-b border-white/10 px-4 py-2.5">
        <h2 className="flex-1 text-sm font-semibold text-stone-200">
          操作日志 · {value?.total ?? 0}
          {caseId ? '（当前案件）' : '（全部案件）'}
        </h2>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
        {loading && (
          <div className="flex items-center gap-2 py-10 text-sm text-stone-500">
            <Loader2 className="h-4 w-4 animate-spin text-amber-300" />
            加载日志…
          </div>
        )}
        {!loading && items.length === 0 && (
          <div className="flex h-full items-center justify-center p-8">
            <p className="max-w-md text-center text-sm leading-6 text-stone-500">暂无操作日志。</p>
          </div>
        )}

        <div className="mx-auto flex max-w-3xl flex-col gap-1.5">
          {items.map((l) => (
            <LogRow key={l.id} log={l} />
          ))}
        </div>
      </div>
    </div>
  )
}

function LogRow({ log }: { log: OperationLog }) {
  return (
    <div className="rounded-xl border border-white/8 bg-white/3 px-3.5 py-2">
      <div className="flex items-center gap-2">
        <span className="rounded-md border border-white/10 px-1.5 py-0.5 font-mono text-[0.6rem] text-stone-400">
          {log.module}
        </span>
        <span className="min-w-0 flex-1 truncate text-sm text-stone-200">
          {log.user_name} · {log.action}
        </span>
        <span className="shrink-0 text-[0.65rem] text-stone-600">{formatDateTime(log.created_at)}</span>
      </div>
      {log.target_title && (
        <p className="mt-0.5 truncate text-xs text-stone-500">目标：{log.target_title}</p>
      )}
    </div>
  )
}
