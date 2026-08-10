import { useState } from 'react'
import clsx from 'clsx'
import {
  ArrowUpRight,
  GitMerge,
  Link2,
  Loader2,
  Paperclip,
  Users,
} from 'lucide-react'
import type { AgentCard, ApprovalItem } from '../lib/agent'
import { mergePersons } from '../lib/api'
import { useWorkspace } from './Workbench'
import { parseStatusLabel, translationStatusLabel } from '../lib/hooks'
import type { DedupeGroup, PersonRelationship, SearchResult, TimelineNode } from '../lib/types'

// ---------- Card shell ----------

function CardShell({
  icon,
  intro,
  children,
}: {
  icon?: React.ReactNode
  intro: string
  children: React.ReactNode
}) {
  return (
    <div className="max-w-2xl overflow-hidden rounded-2xl border border-white/10 bg-white/4">
      <div className="flex items-center gap-2 border-b border-white/8 px-4 py-2.5 text-xs text-stone-400">
        {icon}
        <span className="flex-1">{intro}</span>
      </div>
      <div className="p-2">{children}</div>
    </div>
  )
}

// ---------- Nodes ----------

function NodesCard({ nodes }: { nodes: TimelineNode[] }) {
  const { setSelection } = useWorkspace()
  return (
    <div className="flex flex-col gap-1">
      {nodes.slice(0, 12).map((n) => (
        <button
          key={n.id}
          onClick={() => setSelection({ kind: 'node', node: n })}
          className="flex items-center gap-3 rounded-xl px-2.5 py-2 text-left transition hover:bg-white/6"
        >
          <span className="shrink-0 rounded-lg border border-amber-300/25 bg-amber-300/8 px-2 py-1 font-mono text-[0.7rem] text-amber-200">
            {n.event_time}
          </span>
          <span className="min-w-0 flex-1">
            <span className="block truncate text-sm text-stone-100">{n.title}</span>
            {n.description && (
              <span className="block truncate text-xs text-stone-500">{n.description}</span>
            )}
          </span>
          {n.evidence_links.length > 0 && (
            <span className="flex shrink-0 items-center gap-1 text-[0.65rem] text-stone-500">
              <Link2 className="h-3 w-3" />
              {n.evidence_links.length}
            </span>
          )}
        </button>
      ))}
      {nodes.length > 12 && (
        <p className="px-2.5 py-1 text-xs text-stone-600">还有 {nodes.length - 12} 个节点，可在时间轴视图查看全部。</p>
      )}
    </div>
  )
}

// ---------- Files ----------

function FilesCard({ files }: { files: Array<{ id: string; original_name: string; parse_status: string; translation_status: string; translation_incomplete?: boolean }> }) {
  const { setSelection } = useWorkspace()
  return (
    <div className="flex flex-col gap-1">
      {files.slice(0, 10).map((f) => {
        const parse = parseStatusLabel(f.parse_status)
        const trans = translationStatusLabel(f.translation_status)
        return (
          <button
            key={f.id}
            onClick={() => setSelection({ kind: 'file', file: f as never })}
            className="flex items-center gap-2.5 rounded-xl px-2.5 py-2 text-left transition hover:bg-white/6"
          >
            <Paperclip className="h-4 w-4 shrink-0 text-stone-500" />
            <span className="min-w-0 flex-1 truncate text-sm text-stone-100">{f.original_name}</span>
            <span className="shrink-0 text-[0.65rem] text-stone-500">{parse.label}</span>
            <span className="shrink-0 text-[0.65rem] text-stone-500">{trans.label}</span>
          </button>
        )
      })}
    </div>
  )
}

// ---------- Persons ----------

function PersonsCard({ persons }: { persons: Array<{ id: string; name: string; role_type: string; phone?: string | null; organization?: string | null }> }) {
  const { setSelection } = useWorkspace()
  return (
    <div className="flex flex-col gap-1">
      {persons.map((p) => (
        <button
          key={p.id}
          onClick={() => setSelection({ kind: 'person', person: p as never })}
          className="flex items-center gap-2.5 rounded-xl px-2.5 py-2 text-left transition hover:bg-white/6"
        >
          <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full border border-white/10 bg-stone-900 text-xs text-amber-200">
            {p.name.slice(0, 1)}
          </span>
          <span className="min-w-0 flex-1 truncate text-sm text-stone-100">{p.name}</span>
          <span className="shrink-0 truncate text-xs text-stone-500">{p.role_type}</span>
        </button>
      ))}
    </div>
  )
}

// ---------- Dedupe groups ----------

export function DedupeGroupsCard({
  groups,
  addApprovals,
}: {
  groups: DedupeGroup[]
  addApprovals: (items: ApprovalItem[]) => void
}) {
  const { token, caseId } = useWorkspace()
  const [busyId, setBusyId] = useState<string | null>(null)

  if (!caseId) return null

  const proposeMerge = (group: DedupeGroup, keepId: string, sourceId: string) => {
    setBusyId(group.key)
    const approval: ApprovalItem = {
      // eslint-disable-next-line react-hooks/purity -- id minting in an event handler
      id: `ap-merge-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
      title: `合并人物：${sourceId.slice(0, 8)} → ${keepId.slice(0, 8)}`,
      summary: `${group.reason}。确认后移动关联数据并保留目标人物。`,
      level: 'sensitive',
      execute: async () => {
        const result = await mergePersons(token, caseId, {
          source_person_id: sourceId,
          target_person_id: keepId,
        })
        return result
      },
    }
    addApprovals([approval])
    setBusyId(null)
  }

  return (
    <div className="flex flex-col gap-2">
      {groups.map((g) => (
        <div key={g.key} className="rounded-xl border border-amber-300/20 bg-amber-950/15 p-3">
          <p className="text-xs text-amber-200/90">{g.reason}</p>
          <div className="mt-2 flex flex-wrap items-center gap-1.5">
            {g.persons.map((p, i) => (
              <span key={p.id} className="flex items-center gap-1.5">
                <span className="rounded-lg border border-white/10 bg-stone-900 px-2 py-1 text-xs text-stone-200">
                  {p.name}
                  <span className="ml-1 text-[0.6rem] text-stone-500">{p.roles.join('/')}</span>
                </span>
                {i < g.persons.length - 1 && <span className="text-stone-600">+</span>}
              </span>
            ))}
          </div>
          <div className="mt-2.5 flex gap-1.5">
            <button
              onClick={() => proposeMerge(g, g.persons[0].id, g.persons[1].id)}
              disabled={busyId === g.key || g.persons.length < 2}
              className="flex items-center gap-1 rounded-lg bg-amber-300 px-2.5 py-1.5 text-xs font-semibold text-stone-950 transition hover:bg-amber-200 disabled:opacity-40"
            >
              {busyId === g.key ? <Loader2 className="h-3 w-3 animate-spin" /> : <GitMerge className="h-3 w-3" />}
              合并到 {g.persons[0].name}
            </button>
            <span className="self-center text-[0.65rem] text-stone-600">合并属于敏感写操作，确认后执行</span>
          </div>
        </div>
      ))}
    </div>
  )
}

// ---------- Relationships ----------

function RelationshipsCard({ relationships }: { relationships: PersonRelationship[] }) {
  return (
    <div className="flex flex-col gap-1">
      {relationships.map((r) => (
        <div key={r.id} className="flex items-center gap-2 rounded-xl px-2.5 py-1.5 text-sm">
          <span className="text-stone-200">{r.from_person_name}</span>
          <span className="rounded-full border border-white/10 bg-stone-900 px-2 py-0.5 text-[0.65rem] text-amber-200">
            {r.rel_type}
          </span>
          <span className="text-stone-200">{r.to_person_name}</span>
          {r.rel_detail && <span className="text-xs text-stone-500">· {r.rel_detail}</span>}
        </div>
      ))}
    </div>
  )
}

// ---------- Search results ----------

function SearchResultsCard({ results }: { results: SearchResult[] }) {
  const { setSelection, setFocusNodeId, openView } = useWorkspace()

  const open = (hit: SearchResult) => {
    if (hit.object_type === 'node') {
      setFocusNodeId(hit.object_id)
      openView('timeline')
    } else {
      setSelection({ kind: 'search', hit })
    }
  }

  return (
    <div className="flex flex-col gap-1">
      {results.slice(0, 12).map((r) => (
        <button
          key={`${r.object_type}-${r.object_id}-${r.chunk_id ?? ''}`}
          onClick={() => open(r)}
          className="group flex items-center gap-2.5 rounded-xl px-2.5 py-2 text-left transition hover:bg-white/6"
        >
          <span
            className={clsx(
              'shrink-0 rounded-md border px-1.5 py-0.5 text-[0.6rem] uppercase',
              r.object_type === 'node'
                ? 'border-cyan-300/25 text-cyan-200'
                : r.object_type === 'evidence'
                  ? 'border-amber-300/25 text-amber-200'
                  : r.object_type === 'person'
                    ? 'border-emerald-400/25 text-emerald-200'
                    : 'border-white/15 text-stone-400',
            )}
          >
            {r.object_type}
          </span>
          <span className="min-w-0 flex-1">
            <span className="block truncate text-sm text-stone-100">{r.title}</span>
            {r.highlight && (
              <span
                className="block truncate text-xs text-stone-500"
                dangerouslySetInnerHTML={{ __html: r.highlight }}
              />
            )}
          </span>
          <ArrowUpRight className="h-3.5 w-3.5 shrink-0 text-stone-600 transition group-hover:text-amber-200" />
        </button>
      ))}
    </div>
  )
}

// ---------- Info / error ----------

export function InfoCard({ text }: { text: string }) {
  return (
    <div className="max-w-2xl whitespace-pre-line rounded-2xl border border-white/10 bg-white/4 px-4 py-3 text-sm leading-6 text-stone-300">
      {text}
    </div>
  )
}

// ---------- Dispatcher ----------

export function AgentCardView({
  card,
  addApprovals,
}: {
  card: AgentCard
  addApprovals: (items: ApprovalItem[]) => void
}) {
  switch (card.kind) {
    case 'info':
      return <InfoCard text={card.text} />
    case 'error':
      return (
        <div className="max-w-2xl rounded-2xl border border-red-400/25 bg-red-950/20 px-4 py-3 text-sm text-red-200">
          {card.text}
        </div>
      )
    case 'nodes':
      return (
        <CardShell icon={<Link2 className="h-3.5 w-3.5" />} intro={card.intro}>
          <NodesCard nodes={card.nodes} />
        </CardShell>
      )
    case 'files':
      return (
        <CardShell icon={<Paperclip className="h-3.5 w-3.5" />} intro={card.intro}>
          <FilesCard files={card.files} />
        </CardShell>
      )
    case 'persons':
      return (
        <CardShell icon={<Users className="h-3.5 w-3.5" />} intro={card.intro}>
          <PersonsCard persons={card.persons} />
        </CardShell>
      )
    case 'dedupe':
      return (
        <CardShell icon={<GitMerge className="h-3.5 w-3.5" />} intro={card.intro}>
          <DedupeGroupsCard groups={card.groups} addApprovals={addApprovals} />
        </CardShell>
      )
    case 'relationships':
      return (
        <CardShell icon={<Link2 className="h-3.5 w-3.5" />} intro={card.intro}>
          <RelationshipsCard relationships={card.relationships} />
        </CardShell>
      )
    case 'search':
      return (
        <CardShell icon={<ArrowUpRight className="h-3.5 w-3.5" />} intro={card.intro}>
          <SearchResultsCard results={card.results} />
        </CardShell>
      )
    default:
      return null
  }
}
