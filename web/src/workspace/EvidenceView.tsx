import { useMemo, useState } from 'react'
import clsx from 'clsx'
import {
  ChevronDown,
  FileUp,
  Loader2,
  RefreshCw,
} from 'lucide-react'
import { useWorkspace } from './Workbench'
import { fetchFileChunks, fetchFiles, parseFile } from '../lib/api'
import { formatBytes, formatDateTime, parseStatusLabel, translationStatusLabel, useAsync, usePolling, useToasts } from '../lib/hooks'
import type { EvidenceChunk, EvidenceFileSummary } from '../lib/types'

export function EvidenceView() {
  const { token, caseId, dataTick, bump, setSelection, isReadOnly, onUploadRequested } =
    useWorkspace()
  const { ok, error } = useToasts()
  const { value, loading, refresh } = useAsync(
    () => (caseId ? fetchFiles(token, caseId, 1, 100) : Promise.resolve(null)),
    [token, caseId, dataTick],
  )
  const [expandedId, setExpandedId] = useState<string | null>(null)

  const files = value?.files ?? []
  const hasProcessing = useMemo(
    () => files.some((f) => f.parse_status === 'processing' || f.translation_status === 'processing'),
    [files],
  )
  usePolling(refresh, hasProcessing, 4000)

  if (!caseId) {
    return <CenteredHint text="请先在左上角选择一个案件" />
  }

  const triggerParse = async (f: EvidenceFileSummary) => {
    try {
      await parseFile(token, f.id)
      ok(`已提交解析：${f.original_name}`)
      bump()
    } catch (e) {
      error(e instanceof Error ? e.message : '解析失败')
    }
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-2 border-b border-white/10 px-4 py-2.5">
        <h2 className="flex-1 text-sm font-semibold text-stone-200">
          证据文件 · {value?.total ?? 0}
        </h2>
        {hasProcessing && (
          <span className="flex items-center gap-1.5 text-xs text-cyan-200">
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
            后台处理中，自动刷新…
          </span>
        )}
        {!isReadOnly && (
          <button
            onClick={onUploadRequested}
            className="flex items-center gap-1.5 rounded-lg bg-amber-300 px-3 py-1.5 text-xs font-semibold text-stone-950 transition hover:bg-amber-200"
          >
            <FileUp className="h-3.5 w-3.5" />
            上传证据
          </button>
        )}
        <button
          onClick={refresh}
          className="flex items-center gap-1.5 rounded-lg border border-white/10 px-3 py-1.5 text-xs text-stone-400 transition hover:border-white/25 hover:text-stone-200"
        >
          <RefreshCw className="h-3.5 w-3.5" />
          刷新
        </button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
        {loading && (
          <div className="flex items-center gap-2 py-10 text-sm text-stone-500">
            <Loader2 className="h-4 w-4 animate-spin text-amber-300" />
            加载文件…
          </div>
        )}
        {!loading && files.length === 0 && (
          <CenteredHint
            text={isReadOnly ? '案件暂无证据文件。' : '案件暂无证据文件。点击「上传证据」开始。'}
          />
        )}

        <div className="mx-auto flex max-w-4xl flex-col gap-2">
          {files.map((f) => {
            const parse = parseStatusLabel(f.parse_status)
            const trans = translationStatusLabel(f.translation_status)
            const expanded = expandedId === f.id
            return (
              <div key={f.id} className="overflow-hidden rounded-xl border border-white/10 bg-white/4">
                <button
                  onClick={() => {
                    setExpandedId(expanded ? null : f.id)
                    setSelection({ kind: 'file', file: f })
                  }}
                  className="flex w-full items-center gap-3 px-3.5 py-2.5 text-left transition hover:bg-white/6"
                >
                  <ChevronDown
                    className={clsx('h-4 w-4 shrink-0 text-stone-600 transition', expanded && 'rotate-180')}
                  />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm text-stone-100">{f.original_name}</span>
                    <span className="mt-0.5 block text-[0.65rem] text-stone-600">
                      {formatBytes(f.file_size)} · {formatDateTime(f.created_at)}
                    </span>
                  </span>
                  <StatusPill label={parse.label} tone={parse.tone} />
                  <StatusPill label={trans.label} tone={trans.tone} />
                  {!isReadOnly && f.parse_status !== 'done' && f.parse_status !== 'processing' && (
                    <span
                      role="button"
                      tabIndex={0}
                      onClick={(e) => {
                        e.stopPropagation()
                        void triggerParse(f)
                      }}
                      className="shrink-0 rounded-md border border-amber-300/25 px-2 py-1 text-[0.65rem] text-amber-200 transition hover:bg-amber-300/10"
                    >
                      解析
                    </span>
                  )}
                </button>
                {expanded && <ChunkList fileId={f.id} file={f} />}
              </div>
            )
          })}
        </div>
      </div>
    </div>
  )
}

function ChunkList({ fileId, file }: { fileId: string; file: EvidenceFileSummary }) {
  const { token } = useWorkspace()
  const { value, loading, error } = useAsync(
    () => fetchFileChunks(token, fileId, 1, 200),
    [token, fileId],
  )

  if (file.parse_status !== 'done') {
    return (
      <div className="border-t border-white/10 px-4 py-3 text-xs text-stone-500">
        该文件尚未解析完成，暂时无法查看分块内容。
      </div>
    )
  }
  if (loading) {
    return (
      <div className="flex items-center gap-2 border-t border-white/10 px-4 py-3 text-xs text-stone-500">
        <Loader2 className="h-3.5 w-3.5 animate-spin text-amber-300" />
        加载双语块…
      </div>
    )
  }
  if (error || !value) {
    return (
      <div className="border-t border-white/10 px-4 py-3 text-xs text-red-200">
        {error?.message ?? '加载失败'}
      </div>
    )
  }

  return (
    <div className="flex max-h-80 flex-col gap-2 overflow-y-auto border-t border-white/10 px-4 py-3">
      {value.items.length === 0 && (
        <p className="text-xs text-stone-600">没有可分块内容。</p>
      )}
      {value.items.map((c) => (
        <ChunkRow key={c.id} chunk={c} />
      ))}
    </div>
  )
}

function ChunkRow({ chunk }: { chunk: EvidenceChunk }) {
  return (
    <div className="rounded-lg border border-white/8 bg-stone-950/50 p-2.5">
      <div className="mb-1 flex items-center gap-2 text-[0.6rem] text-stone-600">
        <span>#{chunk.chunk_index + 1}</span>
        <span>{chunk.chunk_kind}</span>
        {chunk.page_number != null && <span>第 {chunk.page_number} 页</span>}
        {chunk.display_label && <span>· {chunk.display_label}</span>}
        {chunk.translated_text && (
          <span className="ml-auto rounded-full border border-emerald-400/20 px-1.5 text-emerald-200">
            双语
          </span>
        )}
      </div>
      <p className="whitespace-pre-wrap text-xs leading-5 text-stone-300">{chunk.source_text}</p>
      {chunk.translated_text && (
        <p className="mt-1.5 whitespace-pre-wrap border-t border-white/5 pt-1.5 text-xs leading-5 text-stone-500">
          {chunk.translated_text}
        </p>
      )}
    </div>
  )
}

function StatusPill({ label, tone }: { label: string; tone: 'ok' | 'run' | 'warn' }) {
  return (
    <span
      className={clsx(
        'shrink-0 rounded-full border px-2 py-0.5 text-[0.65rem]',
        tone === 'ok'
          ? 'border-emerald-400/25 bg-emerald-400/10 text-emerald-100'
          : tone === 'run'
            ? 'border-cyan-300/25 bg-cyan-300/10 text-cyan-100'
            : 'border-amber-300/25 bg-amber-300/10 text-amber-100',
      )}
    >
      {label}
    </span>
  )
}

function CenteredHint({ text }: { text: string }) {
  return (
    <div className="flex h-full items-center justify-center p-8">
      <p className="max-w-md text-center text-sm leading-6 text-stone-500">{text}</p>
    </div>
  )
}
