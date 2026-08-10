import { useState } from 'react'
import clsx from 'clsx'
import {
  AlertTriangle,
  Check,
  Download,
  Link2,
  Loader2,
  Paperclip,
  RefreshCw,
  Unlink,
  X,
} from 'lucide-react'
import { useWorkspace } from './Workbench'
import type { ApprovalItem } from '../lib/agent'
import { parseFile, unlinkEvidenceFromNode, updateNode, downloadFile } from '../lib/api'
import { formatBytes, formatDate, parseStatusLabel, translationStatusLabel } from '../lib/hooks'
import { useToasts } from '../lib/hooks'

// ---------- Approval queue ----------

export function ContextPanel({
  approvals,
  onConfirm,
  onReject,
}: {
  approvals: ApprovalItem[]
  onConfirm: (a: ApprovalItem) => void
  onReject: (a: ApprovalItem) => void
}) {
  const { selection } = useWorkspace()

  return (
    <aside className="flex w-[21rem] shrink-0 flex-col border-l border-white/10 bg-stone-950/70">
      {/* Approval queue */}
      <div className="border-b border-white/10">
        <div className="flex items-center justify-between px-4 pb-2 pt-3">
          <h2 className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-stone-400">
            <AlertTriangle className="h-3.5 w-3.5 text-amber-300" />
            待确认动作
            {approvals.length > 0 && (
              <span className="rounded-full bg-amber-300 px-1.5 text-[0.65rem] font-bold text-stone-950">
                {approvals.length}
              </span>
            )}
          </h2>
        </div>

        {approvals.length === 0 ? (
          <p className="px-4 pb-3 text-xs leading-5 text-stone-600">
            写操作（新建、修改、删除、导出）都会先出现在这里，确认后才真正执行。
          </p>
        ) : (
          <div className="flex max-h-72 flex-col gap-2 overflow-y-auto px-3 pb-3">
            {approvals.map((a) => (
              <ApprovalRow key={a.id} approval={a} onConfirm={onConfirm} onReject={onReject} />
            ))}
          </div>
        )}
      </div>

      {/* Selected object detail */}
      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
        <h2 className="pb-2 text-xs font-semibold uppercase tracking-wider text-stone-500">
          上下文
        </h2>
        {selection ? (
          <SelectionDetail />
        ) : (
          <p className="text-xs leading-5 text-stone-600">
            在时间轴、证据或人物视图中选中对象后，这里会显示详情与快捷操作。
          </p>
        )}
      </div>
    </aside>
  )
}

function ApprovalRow({
  approval,
  onConfirm,
  onReject,
}: {
  approval: ApprovalItem
  onConfirm: (a: ApprovalItem) => void
  onReject: (a: ApprovalItem) => void
}) {
  const [busy, setBusy] = useState(false)

  const confirm = async () => {
    setBusy(true)
    try {
      await onConfirm(approval)
    } finally {
      setBusy(false)
    }
  }

  return (
    <div
      className={clsx(
        'rounded-xl border p-3',
        approval.level === 'destructive'
          ? 'border-red-400/25 bg-red-950/20'
          : approval.level === 'sensitive'
            ? 'border-amber-300/30 bg-amber-950/20'
            : 'border-white/10 bg-white/4',
      )}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="truncate text-sm font-medium text-stone-100">{approval.title}</p>
          <p className="mt-0.5 line-clamp-2 text-xs text-stone-400">{approval.summary}</p>
        </div>
        <span
          className={clsx(
            'shrink-0 rounded-full border px-1.5 py-0.5 text-[0.6rem] uppercase',
            approval.level === 'destructive'
              ? 'border-red-400/30 text-red-200'
              : approval.level === 'sensitive'
                ? 'border-amber-300/30 text-amber-200'
                : 'border-emerald-400/25 text-emerald-200',
          )}
        >
          {approval.level === 'destructive' ? 'destructive' : approval.level === 'sensitive' ? 'sensitive' : 'write'}
        </span>
      </div>
      <div className="mt-2.5 flex gap-2">
        <button
          onClick={confirm}
          disabled={busy}
          className="flex flex-1 items-center justify-center gap-1 rounded-lg bg-amber-300 py-1.5 text-xs font-semibold text-stone-950 transition hover:bg-amber-200 disabled:opacity-50"
        >
          {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Check className="h-3.5 w-3.5" />}
          确认执行
        </button>
        <button
          onClick={() => onReject(approval)}
          disabled={busy}
          className="flex items-center gap-1 rounded-lg border border-white/10 px-3 py-1.5 text-xs text-stone-400 transition hover:border-white/25 hover:text-stone-200 disabled:opacity-50"
        >
          <X className="h-3.5 w-3.5" />
          取消
        </button>
      </div>
    </div>
  )
}

// ---------- Selection detail ----------

function SelectionDetail() {
  const { selection, isReadOnly, bump } = useWorkspace()
  const { ok, error } = useToasts()

  if (selection?.kind === 'node') return <NodeDetail node={selection.node} onDirty={bump} readOnly={isReadOnly} />
  if (selection?.kind === 'file')
    return (
      <FileDetail
        key={selection.file.id}
        file={selection.file}
        onDirty={bump}
        readOnly={isReadOnly}
        ok={ok}
        error={error}
      />
    )
  if (selection?.kind === 'person') return <PersonDetail person={selection.person} />
  if (selection?.kind === 'search')
    return (
      <div className="rounded-xl border border-white/10 bg-white/4 p-3">
        <p className="text-xs text-stone-500">
          {selection.hit.object_type} · 命中「{selection.hit.title}」
        </p>
        {selection.hit.highlight && (
          <p
            className="mt-2 text-sm leading-6 text-stone-300"
            // The backend returns <mark> tags in highlights.
            dangerouslySetInnerHTML={{ __html: selection.hit.highlight }}
          />
        )}
      </div>
    )
  return null
}

function NodeDetail({
  node,
  onDirty,
  readOnly,
}: {
  node: { id: string; title: string; description?: string | null; event_time: string; evidence_links: Array<{ id: string; evidence_id: string }> }
  onDirty: () => void
  readOnly: boolean
}) {
  const { token, setSelection } = useWorkspace()
  const { ok, error } = useToasts()
  const [title, setTitle] = useState(node.title)
  const [date, setDate] = useState(node.event_time)
  const [description, setDescription] = useState(node.description ?? '')
  const [saving, setSaving] = useState(false)

  const save = async () => {
    setSaving(true)
    try {
      await updateNode(token, node.id, {
        title: title.trim(),
        event_time: date,
        description: description.trim() || undefined,
      })
      ok('节点已更新')
      onDirty()
    } catch (e) {
      error(e instanceof Error ? e.message : '更新失败')
    } finally {
      setSaving(false)
    }
  }

  const unlink = async (linkId: string) => {
    try {
      await unlinkEvidenceFromNode(token, node.id, linkId)
      ok('已解除证据关联')
      onDirty()
      setSelection(null)
    } catch (e) {
      error(e instanceof Error ? e.message : '解除关联失败')
    }
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="rounded-xl border border-white/10 bg-white/4 p-3">
        <p className="pb-2 text-xs text-stone-500">时间轴节点</p>
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          disabled={readOnly}
          className="w-full rounded-lg border border-white/10 bg-stone-900 px-3 py-2 text-sm outline-none focus:border-amber-300/50"
          placeholder="标题"
        />
        <input
          value={date}
          onChange={(e) => setDate(e.target.value)}
          type="date"
          disabled={readOnly}
          className="mt-2 w-full rounded-lg border border-white/10 bg-stone-900 px-3 py-2 text-sm outline-none focus:border-amber-300/50"
        />
        <textarea
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          disabled={readOnly}
          rows={3}
          className="mt-2 w-full resize-none rounded-lg border border-white/10 bg-stone-900 px-3 py-2 text-sm outline-none focus:border-amber-300/50"
          placeholder="描述"
        />
        {!readOnly && (
          <button
            onClick={save}
            disabled={saving}
            className="mt-2 flex w-full items-center justify-center gap-1.5 rounded-lg bg-amber-300 py-2 text-sm font-semibold text-stone-950 transition hover:bg-amber-200 disabled:opacity-50"
          >
            {saving && <Loader2 className="h-4 w-4 animate-spin" />}
            保存修改
          </button>
        )}
      </div>

      <div className="rounded-xl border border-white/10 bg-white/4 p-3">
        <p className="flex items-center gap-1.5 pb-2 text-xs text-stone-500">
          <Link2 className="h-3.5 w-3.5" />
          关联证据（{node.evidence_links.length}）
        </p>
        {node.evidence_links.length === 0 ? (
          <p className="text-xs text-stone-600">尚未关联证据。可在对话中告诉 Agent 关联哪个文件。</p>
        ) : (
          <ul className="flex flex-col gap-1.5">
            {node.evidence_links.map((l) => (
              <li key={l.id} className="flex items-center justify-between gap-2 rounded-lg bg-stone-900 px-2.5 py-1.5 text-xs">
                <span className="truncate text-stone-300">{l.evidence_id}</span>
                {!readOnly && (
                  <button
                    onClick={() => unlink(l.id)}
                    className="shrink-0 text-stone-500 transition hover:text-red-300"
                    title="解除关联"
                  >
                    <Unlink className="h-3.5 w-3.5" />
                  </button>
                )}
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  )
}

function FileDetail({
  file,
  onDirty,
  readOnly,
  ok,
  error,
}: {
  file: {
    id: string
    original_name: string
    parse_status: string
    translation_status: string
    file_size: number
    created_at: string
    parse_error?: string | null
  }
  onDirty: () => void
  readOnly: boolean
  ok: (t: string) => void
  error: (t: string) => void
}) {
  const { token } = useWorkspace()
  const [busy, setBusy] = useState(false)
  const parse = parseStatusLabel(file.parse_status)
  const trans = translationStatusLabel(file.translation_status)

  const triggerParse = async () => {
    setBusy(true)
    try {
      await parseFile(token, file.id)
      ok(`已提交解析：${file.original_name}`)
      onDirty()
    } catch (e) {
      error(e instanceof Error ? e.message : '解析失败')
    } finally {
      setBusy(false)
    }
  }

  const download = async () => {
    try {
      const blob = await downloadFile(token, file.id)
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = file.original_name
      a.click()
      URL.revokeObjectURL(url)
    } catch (e) {
      error(e instanceof Error ? e.message : '下载失败')
    }
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="rounded-xl border border-white/10 bg-white/4 p-3">
        <p className="flex items-center gap-1.5 pb-2 text-xs text-stone-500">
          <Paperclip className="h-3.5 w-3.5" />
          证据文件
        </p>
        <p className="break-all text-sm font-medium text-stone-100">{file.original_name}</p>
        <p className="mt-1 text-xs text-stone-500">
          {formatBytes(file.file_size)} · {formatDate(file.created_at)}
        </p>
        <div className="mt-2 flex flex-wrap gap-1.5">
          <StatusBadge label={parse.label} tone={parse.tone} />
          <StatusBadge label={trans.label} tone={trans.tone} />
        </div>
        {file.parse_error && (
          <p className="mt-2 rounded-lg bg-red-950/40 px-2.5 py-1.5 text-xs text-red-200">
            {file.parse_error}
          </p>
        )}
        {!readOnly && file.parse_status !== 'done' && (
          <button
            onClick={triggerParse}
            disabled={busy || file.parse_status === 'processing'}
            className="mt-2.5 flex w-full items-center justify-center gap-1.5 rounded-lg bg-amber-300 py-2 text-sm font-semibold text-stone-950 transition hover:bg-amber-200 disabled:opacity-50"
          >
            {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <RefreshCw className="h-4 w-4" />}
            {file.parse_status === 'processing' ? '解析中…' : '开始解析'}
          </button>
        )}
        <button
          onClick={download}
          className="mt-2 flex w-full items-center justify-center gap-1.5 rounded-lg border border-white/10 py-2 text-sm text-stone-300 transition hover:border-white/25"
        >
          <Download className="h-4 w-4" />
          下载原件
        </button>
      </div>
    </div>
  )
}

function PersonDetail({ person }: { person: { name: string; role_type: string; phone?: string | null; email?: string | null; organization?: string | null; position?: string | null } }) {
  return (
    <div className="rounded-xl border border-white/10 bg-white/4 p-3">
      <p className="pb-2 text-xs text-stone-500">人物</p>
      <p className="text-sm font-medium text-stone-100">{person.name}</p>
      <p className="mt-0.5 text-xs text-stone-400">{person.role_type}</p>
      <dl className="mt-2 flex flex-col gap-1 text-xs text-stone-400">
        {person.organization && <dd>机构：{person.organization}</dd>}
        {person.position && <dd>职位：{person.position}</dd>}
        {person.phone && <dd>电话：{person.phone}</dd>}
        {person.email && <dd>邮箱：{person.email}</dd>}
      </dl>
    </div>
  )
}

function StatusBadge({ label, tone }: { label: string; tone: 'ok' | 'run' | 'warn' }) {
  return (
    <span
      className={clsx(
        'rounded-full border px-2 py-0.5 text-[0.65rem]',
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
