import { useEffect, useMemo, useRef, useState } from 'react'
import clsx from 'clsx'
import { Loader2, Plus, RefreshCw } from 'lucide-react'
import { useWorkspace } from './Workbench'
import { createNode, deleteNode, fetchNodes } from '../lib/api'
import { useAsync, useToasts } from '../lib/hooks'
import type { TimelineNode } from '../lib/types'

export function TimelineView() {
  const { token, caseId, dataTick, bump, setSelection, isReadOnly, focusNodeId, setFocusNodeId } =
    useWorkspace()
  const { ok, error } = useToasts()
  const { value, loading, error: loadError, refresh } = useAsync(
    () => (caseId ? fetchNodes(token, caseId, 1, 500) : Promise.resolve(null)),
    [token, caseId, dataTick],
  )

  const [showCreate, setShowCreate] = useState(false)

  const nodes = value?.nodes ?? []
  const months = useMemo(() => {
    const map = new Map<string, TimelineNode[]>()
    for (const n of nodes) {
      const key = n.event_time.slice(0, 7)
      const arr = map.get(key) ?? []
      arr.push(n)
      map.set(key, arr)
    }
    return [...map.entries()].sort(([a], [b]) => a.localeCompare(b))
  }, [nodes])

  const focusRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (focusNodeId && focusRef.current) {
      focusRef.current.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'center' })
      const t = window.setTimeout(() => setFocusNodeId(null), 2500)
      return () => window.clearTimeout(t)
    }
  }, [focusNodeId, setFocusNodeId, nodes])

  if (!caseId) {
    return <CenteredHint text="请先在左上角选择一个案件" />
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-2 border-b border-white/10 px-4 py-2.5">
        <h2 className="flex-1 text-sm font-semibold text-stone-200">时间轴 · {nodes.length} 个节点</h2>
        {!isReadOnly && (
          <button
            onClick={() => setShowCreate((v) => !v)}
            className="flex items-center gap-1.5 rounded-lg bg-amber-300 px-3 py-1.5 text-xs font-semibold text-stone-950 transition hover:bg-amber-200"
          >
            <Plus className="h-3.5 w-3.5" />
            新建节点
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

      {showCreate && !isReadOnly && (
        <CreateNodeForm
          caseId={caseId}
          token={token}
          onDone={() => {
            setShowCreate(false)
            bump()
          }}
        />
      )}

      <div className="min-h-0 flex-1 overflow-auto px-4 py-4">
        {loading && (
          <div className="flex items-center gap-2 py-10 text-sm text-stone-500">
            <Loader2 className="h-4 w-4 animate-spin text-amber-300" />
            加载节点…
          </div>
        )}
        {loadError && <CenteredHint text={`加载失败：${loadError.message}`} />}
        {!loading && !loadError && nodes.length === 0 && (
          <CenteredHint text="还没有时间轴节点。点击「新建节点」，或在对话中让 Agent 帮你创建。" />
        )}

        {!loading && nodes.length > 0 && (
          <div className="flex flex-col gap-6">
            <div className="relative border-l border-white/10 pl-6">
              <div className="absolute -left-px top-0 h-full w-px bg-gradient-to-b from-amber-300/40 via-white/10 to-transparent" />
              {months.map(([month, monthNodes]) => (
                <div
                  key={month}
                  ref={month === focusNodeId?.slice(0, 7) ? focusRef : undefined}
                  className="relative pb-7"
                >
                  <div className="absolute -left-[1.7rem] flex h-6 w-6 items-center justify-center rounded-full border border-amber-300/30 bg-stone-950">
                    <span className="h-1.5 w-1.5 rounded-full bg-amber-300" />
                  </div>
                  <p className="pb-2 font-mono text-xs uppercase tracking-wider text-stone-500">
                    {month}
                  </p>
                  <div className="flex flex-col gap-1.5">
                    {monthNodes.map((n) => {
                      const focused = focusNodeId === n.id
                      return (
                        <button
                          key={n.id}
                          onClick={() => setSelection({ kind: 'node', node: n })}
                          className={clsx(
                            'group flex items-start gap-3 rounded-xl border px-3 py-2.5 text-left transition',
                            focused
                              ? 'border-amber-300/60 bg-amber-300/12'
                              : 'border-white/10 bg-white/4 hover:border-amber-300/30 hover:bg-amber-300/6',
                          )}
                        >
                          <span className="mt-0.5 shrink-0 font-mono text-[0.7rem] text-amber-200/80">
                            {n.event_time.slice(5)}
                          </span>
                          <span className="min-w-0 flex-1">
                            <span className="block text-sm font-medium text-stone-100">{n.title}</span>
                            {n.description && (
                              <span className="mt-0.5 block truncate text-xs text-stone-500">
                                {n.description}
                              </span>
                            )}
                            {n.evidence_links.length > 0 && (
                              <span className="mt-1 inline-block rounded-full border border-amber-300/20 bg-amber-300/8 px-2 py-0.5 text-[0.6rem] text-amber-200">
                                {n.evidence_links.length} 条证据关联
                              </span>
                            )}
                          </span>
                          <span className="mt-1 flex shrink-0 gap-1 opacity-0 transition group-hover:opacity-100">
                            {!isReadOnly && (
                              <span
                                role="button"
                                tabIndex={0}
                                onClick={(e) => {
                                  e.stopPropagation()
                                  void (async () => {
                                    try {
                                      await deleteNode(token, n.id)
                                      ok('节点已删除')
                                      bump()
                                    } catch (err) {
                                      error(err instanceof Error ? err.message : '删除失败')
                                    }
                                  })()
                                }}
                                className="rounded-md border border-white/10 px-1.5 py-0.5 text-[0.6rem] text-stone-500 hover:border-red-400/40 hover:text-red-300"
                              >
                                删除
                              </span>
                            )}
                          </span>
                        </button>
                      )
                    })}
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

function CreateNodeForm({
  caseId,
  token,
  onDone,
}: {
  caseId: string
  token: string
  onDone: () => void
}) {
  const { ok, error } = useToasts()
  const [title, setTitle] = useState('')
  const [date, setDate] = useState('')
  const [description, setDescription] = useState('')
  const [busy, setBusy] = useState(false)

  const submit = async () => {
    if (!title.trim() || !date) {
      error('标题与日期必填')
      return
    }
    setBusy(true)
    try {
      await createNode(token, caseId, {
        title: title.trim(),
        event_time: date,
        description: description.trim() || undefined,
      })
      ok('节点已创建')
      onDone()
    } catch (e) {
      error(e instanceof Error ? e.message : '创建失败')
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="shrink-0 border-b border-white/10 bg-white/3 px-4 py-3">
      <div className="mx-auto flex max-w-2xl flex-col gap-2">
        <div className="flex gap-2">
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="节点标题"
            className="min-w-0 flex-1 rounded-lg border border-white/10 bg-stone-900 px-3 py-2 text-sm outline-none focus:border-amber-300/50"
          />
          <input
            value={date}
            onChange={(e) => setDate(e.target.value)}
            type="date"
            className="rounded-lg border border-white/10 bg-stone-900 px-3 py-2 text-sm outline-none focus:border-amber-300/50"
          />
        </div>
        <input
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="描述（可选）"
          className="rounded-lg border border-white/10 bg-stone-900 px-3 py-2 text-sm outline-none focus:border-amber-300/50"
        />
        <div className="flex gap-2">
          <button
            onClick={submit}
            disabled={busy}
            className="flex items-center gap-1.5 rounded-lg bg-amber-300 px-4 py-1.5 text-xs font-semibold text-stone-950 transition hover:bg-amber-200 disabled:opacity-50"
          >
            {busy && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
            创建
          </button>
          <button
            onClick={onDone}
            className="rounded-lg border border-white/10 px-4 py-1.5 text-xs text-stone-400 transition hover:text-stone-200"
          >
            取消
          </button>
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
