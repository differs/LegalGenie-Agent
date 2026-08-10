import { useState } from 'react'
import clsx from 'clsx'
import { Search as SearchIcon } from 'lucide-react'
import { useWorkspace } from './Workbench'
import { fetchSearchSuggestions, search } from '../lib/api'
import { useAsync, useToasts } from '../lib/hooks'
import type { SearchResult } from '../lib/types'

const OBJECT_TYPES = ['nodes', 'evidence', 'persons'] as const

export function SearchView() {
  const { token, caseId, onNeedCase } = useWorkspace()
  const { error: toastError } = useToasts()
  const [q, setQ] = useState('')
  const [submitted, setSubmitted] = useState('')
  const [objectTypes, setObjectTypes] = useState<string[]>([])
  const [results, setResults] = useState<SearchResult[] | null>(null)

  const { value: suggestions, refresh: refreshSuggestions } = useAsync(
    () => (q.trim().length >= 1 ? fetchSearchSuggestions(token, q) : Promise.resolve([])),
    [token, q],
  )

  const runSearch = async (keyword: string) => {
    if (!caseId) {
      onNeedCase()
      return
    }
    setSubmitted(keyword)
    setResults(null)
    try {
      const data = await search(
        token,
        keyword,
        objectTypes.length ? objectTypes : undefined,
        caseId,
      )
      setResults(data.results)
    } catch (e) {
      toastError(e instanceof Error ? e.message : '搜索失败')
      setResults([])
    }
  }

  const toggleType = (t: string) => {
    setObjectTypes((prev) =>
      prev.includes(t) ? prev.filter((x) => x !== t) : [...prev, t],
    )
  }

  return (
    <div className="flex h-full flex-col">
      <div className="shrink-0 border-b border-white/10 px-4 py-3">
        <div className="mx-auto flex max-w-2xl items-center gap-2">
          <div className="flex min-w-0 flex-1 items-center gap-2 rounded-xl border border-white/10 bg-stone-900 px-3.5 focus-within:border-amber-300/40">
            <SearchIcon className="h-4 w-4 shrink-0 text-stone-500" />
            <input
              value={q}
              onChange={(e) => {
                setQ(e.target.value)
                refreshSuggestions()
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.nativeEvent.isComposing) {
                  e.preventDefault()
                  void runSearch(q.trim())
                }
              }}
              placeholder="搜索案件材料：关键词、日期、人名…"
              className="w-full bg-transparent py-2.5 text-sm text-stone-100 outline-none placeholder:text-stone-600"
            />
            <button
              onClick={() => void runSearch(q.trim())}
              disabled={!q.trim()}
              className="shrink-0 rounded-lg bg-amber-300 px-3 py-1.5 text-xs font-semibold text-stone-950 transition hover:bg-amber-200 disabled:opacity-40"
            >
              搜索
            </button>
          </div>
        </div>

        {suggestions && suggestions.length > 0 && (
          <div className="mx-auto flex max-w-2xl flex-wrap gap-1.5 pt-2">
            {suggestions.map((s) => (
              <button
                key={s}
                onClick={() => {
                  setQ(s)
                  void runSearch(s)
                }}
                className="rounded-full border border-white/10 px-2.5 py-1 text-xs text-stone-400 transition hover:border-amber-300/30 hover:text-amber-200"
              >
                {s}
              </button>
            ))}
          </div>
        )}

        <div className="mx-auto flex max-w-2xl items-center gap-2 pt-2">
          <span className="text-[0.65rem] uppercase tracking-wider text-stone-600">范围</span>
          {OBJECT_TYPES.map((t) => (
            <button
              key={t}
              onClick={() => toggleType(t)}
              className={clsx(
                'rounded-full border px-2.5 py-0.5 text-xs transition',
                objectTypes.includes(t)
                  ? 'border-amber-300/40 bg-amber-300/10 text-amber-200'
                  : 'border-white/10 text-stone-500 hover:text-stone-300',
              )}
            >
              {t}
            </button>
          ))}
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
        {!submitted && !results && (
          <CenteredHint text="输入关键词搜索当前案件的节点、证据与人物。默认启用中英文与双语块检索。" />
        )}

        {results !== null && (
          <div className="mx-auto flex max-w-2xl flex-col gap-1.5">
            <p className="pb-1 text-xs text-stone-500">
              「{submitted}」 · {results.length} 条结果
            </p>
            {results.length === 0 && <CenteredHint text="没有匹配结果，换个关键词试试。" />}
            {results.map((r) => (
              <ResultRow key={`${r.object_type}-${r.object_id}-${r.chunk_id ?? ''}`} hit={r} />
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

function ResultRow({ hit }: { hit: SearchResult }) {
  const { setSelection, setFocusNodeId, openView } = useWorkspace()

  const open = () => {
    if (hit.object_type === 'node') {
      setFocusNodeId(hit.object_id)
      openView('timeline')
    } else {
      setSelection({ kind: 'search', hit })
    }
  }

  return (
    <button
      onClick={open}
      className="rounded-xl border border-white/10 bg-white/4 px-3.5 py-2.5 text-left transition hover:border-amber-300/30 hover:bg-amber-300/6"
    >
      <div className="flex items-center gap-2">
        <span
          className={clsx(
            'shrink-0 rounded-md border px-1.5 py-0.5 text-[0.6rem] uppercase',
            hit.object_type === 'node'
              ? 'border-cyan-300/25 text-cyan-200'
              : hit.object_type === 'evidence'
                ? 'border-amber-300/25 text-amber-200'
                : 'border-emerald-400/25 text-emerald-200',
          )}
        >
          {hit.object_type}
        </span>
        <span className="min-w-0 flex-1 truncate text-sm text-stone-100">{hit.title}</span>
        <span className="shrink-0 text-[0.65rem] text-stone-600">
          {hit.score.toFixed(2)}
        </span>
      </div>
      {hit.highlight && (
        <p
          className="mt-1 truncate text-xs text-stone-500"
          dangerouslySetInnerHTML={{ __html: hit.highlight }}
        />
      )}
      {hit.chunk_index != null && (
        <p className="mt-1 text-[0.65rem] text-stone-600">
          {hit.file_name ?? ''} #{hit.chunk_index + 1}
          {hit.matched_language ? ` · ${hit.matched_language}` : ''}
        </p>
      )}
    </button>
  )
}

function CenteredHint({ text }: { text: string }) {
  return (
    <div className="flex h-full items-center justify-center p-8">
      <p className="max-w-md text-center text-sm leading-6 text-stone-500">{text}</p>
    </div>
  )
}
