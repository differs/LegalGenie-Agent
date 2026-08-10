import { useState } from 'react'
import { Download, FileSpreadsheet, FileText, Image as ImageIcon, Loader2 } from 'lucide-react'
import { useWorkspace } from './Workbench'
import {
  downloadExport,
  exportEvidenceList,
  exportTimeline,
  exportTimelineReport,
  fetchExportHistory,
} from '../lib/api'
import { formatBytes, formatDateTime, useAsync, useToasts } from '../lib/hooks'

export function ExportsView() {
  const { token, caseId, dataTick, isReadOnly } = useWorkspace()
  const { ok, error } = useToasts()
  const { value, loading, refresh } = useAsync(
    () => (caseId ? fetchExportHistory(token, caseId) : Promise.resolve(null)),
    [token, caseId, dataTick],
  )
  const [busy, setBusy] = useState<string | null>(null)

  if (!caseId) {
    return <CenteredHint text="请先在左上角选择一个案件" />
  }

  const run = async (key: string, fn: () => Promise<unknown>) => {
    setBusy(key)
    try {
      await fn()
      ok('导出任务已生成')
      refresh()
    } catch (e) {
      error(e instanceof Error ? e.message : '导出失败')
    } finally {
      setBusy(null)
    }
  }

  const saveBlob = async (key: string, fn: () => Promise<Blob>) => {
    setBusy(key)
    try {
      const blob = await fn()
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `legalgenie-${key}.${key.includes('csv') ? 'csv' : 'xlsx'}`
      a.click()
      URL.revokeObjectURL(url)
      ok('下载完成')
    } catch (e) {
      error(e instanceof Error ? e.message : '下载失败')
    } finally {
      setBusy(null)
    }
  }

  const items = value?.items ?? []

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-2 border-b border-white/10 px-4 py-2.5">
        <h2 className="flex-1 text-sm font-semibold text-stone-200">导出 · {items.length}</h2>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4">
        <div className="mx-auto flex max-w-3xl flex-col gap-4">
          <div className="rounded-2xl border border-white/10 bg-white/4 p-4">
            <p className="pb-3 text-xs font-semibold uppercase tracking-wider text-stone-500">
              生成新导出
            </p>
            <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
              <button
                onClick={() =>
                  void run('evidence-list', () => exportEvidenceList(token, caseId))
                }
                disabled={busy !== null || isReadOnly}
                className="flex flex-col items-start gap-2 rounded-xl border border-white/10 bg-stone-900 p-3.5 text-left transition hover:border-amber-300/40 disabled:opacity-40"
              >
                {busy === 'evidence-list' ? (
                  <Loader2 className="h-5 w-5 animate-spin text-amber-300" />
                ) : (
                  <FileSpreadsheet className="h-5 w-5 text-amber-200" />
                )}
                <span className="text-sm font-medium text-stone-100">证据清单</span>
                <span className="text-xs text-stone-500">全部证据汇总为 xlsx</span>
              </button>
              <button
                onClick={() =>
                  void run('timeline-report', () => exportTimelineReport(token, caseId, 'pdf'))
                }
                disabled={busy !== null || isReadOnly}
                className="flex flex-col items-start gap-2 rounded-xl border border-white/10 bg-stone-900 p-3.5 text-left transition hover:border-amber-300/40 disabled:opacity-40"
              >
                {busy === 'timeline-report' ? (
                  <Loader2 className="h-5 w-5 animate-spin text-amber-300" />
                ) : (
                  <FileText className="h-5 w-5 text-amber-200" />
                )}
                <span className="text-sm font-medium text-stone-100">时间轴报告</span>
                <span className="text-xs text-stone-500">节点与证据关联汇总 PDF</span>
              </button>
              <button
                onClick={() =>
                  void run('timeline-png', () => exportTimeline(token, caseId, 'png'))
                }
                disabled={busy !== null || isReadOnly}
                className="flex flex-col items-start gap-2 rounded-xl border border-white/10 bg-stone-900 p-3.5 text-left transition hover:border-amber-300/40 disabled:opacity-40"
              >
                {busy === 'timeline-png' ? (
                  <Loader2 className="h-5 w-5 animate-spin text-amber-300" />
                ) : (
                  <ImageIcon className="h-5 w-5 text-amber-200" />
                )}
                <span className="text-sm font-medium text-stone-100">时间轴图片</span>
                <span className="text-xs text-stone-500">画布快照 PNG</span>
              </button>
            </div>
          </div>

          <div className="rounded-2xl border border-white/10 bg-white/4 p-4">
            <p className="pb-3 text-xs font-semibold uppercase tracking-wider text-stone-500">
              历史导出
            </p>
            {loading && (
              <div className="flex items-center gap-2 py-6 text-sm text-stone-500">
                <Loader2 className="h-4 w-4 animate-spin text-amber-300" />
                加载中…
              </div>
            )}
            {!loading && items.length === 0 && (
              <p className="py-6 text-center text-sm text-stone-600">暂无导出记录。</p>
            )}
            <div className="flex flex-col gap-1.5">
              {items.map((r) => (
                <div
                  key={r.id}
                  className="flex items-center gap-3 rounded-xl border border-white/10 bg-stone-950/50 px-3.5 py-2.5"
                >
                  <FileText className="h-4 w-4 shrink-0 text-stone-500" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm text-stone-200">{r.file_name}</span>
                    <span className="block text-[0.65rem] text-stone-600">
                      {r.export_type} · {formatBytes(r.file_size)} · {formatDateTime(r.generated_at)}
                    </span>
                  </span>
                  <button
                    onClick={() =>
                      void saveBlob(r.export_type, () => downloadExport(token, caseId, r.id))
                    }
                    disabled={busy !== null}
                    className="flex items-center gap-1.5 rounded-lg border border-white/10 px-2.5 py-1.5 text-xs text-stone-400 transition hover:border-amber-300/40 hover:text-amber-200 disabled:opacity-40"
                  >
                    <Download className="h-3.5 w-3.5" />
                    下载
                  </button>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

function CenteredHint({ text }: { text: string }) {
  return (
    <div className="flex h-full items-center justify-center p-8">
      <p className="max-w-md text-center text-sm leading-6 text-stone-500">{text}</p>
    </div>
  )
}
