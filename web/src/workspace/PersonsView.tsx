import { useState } from 'react'
import clsx from 'clsx'
import {
  GitMerge,
  Link2,
  Loader2,
  Plus,
  RefreshCw,
  Trash2,
  Users,
} from 'lucide-react'
import { useWorkspace } from './Workbench'
import {
  createPerson,
  createRelationship,
  deleteRelationship,
  fetchDedupeCandidates,
  fetchPersons,
  fetchRelationships,
  mergePersons,
} from '../lib/api'
import { useAsync, useToasts } from '../lib/hooks'
import type { CasePerson } from '../lib/types'

type Tab = 'list' | 'dedupe'

export function PersonsView() {
  const { token, caseId, dataTick, bump, setSelection, isReadOnly, addApprovals } = useWorkspace()
  const { ok, error } = useToasts()
  const [tab, setTab] = useState<Tab>('list')
  const [showCreate, setShowCreate] = useState(false)

  const { value: personsData, loading, refresh } = useAsync(
    () => (caseId ? fetchPersons(token, caseId, 1, 200) : Promise.resolve(null)),
    [token, caseId, dataTick],
  )
  const { value: relData, refresh: refreshRels } = useAsync(
    () => (caseId ? fetchRelationships(token, caseId) : Promise.resolve(null)),
    [token, caseId, dataTick],
  )
  const { value: dedupeData, refresh: refreshDedupe } = useAsync(
    () => (caseId ? fetchDedupeCandidates(token, caseId) : Promise.resolve(null)),
    [token, caseId, dataTick],
  )

  const persons = personsData?.persons ?? []

  if (!caseId) {
    return <CenteredHint text="请先在左上角选择一个案件" />
  }

  const refreshAll = () => {
    refresh()
    refreshRels()
    refreshDedupe()
  }

  const createRel = async (from: string, to: string, relType: string, relDetail?: string) => {
    try {
      await createRelationship(token, caseId, {
        from_person_id: from,
        to_person_id: to,
        rel_type: relType,
        rel_detail: relDetail,
      })
      ok('关系已创建')
      refreshAll()
    } catch (e) {
      error(e instanceof Error ? e.message : '创建关系失败')
    }
  }

  const removeRel = async (id: string) => {
    try {
      await deleteRelationship(token, caseId, id)
      ok('关系已删除')
      refreshAll()
    } catch (e) {
      error(e instanceof Error ? e.message : '删除关系失败')
    }
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-2 border-b border-white/10 px-4 py-2.5">
        <h2 className="flex-1 text-sm font-semibold text-stone-200">人物 · {persons.length}</h2>
        <div className="flex rounded-full border border-white/10 bg-stone-900 p-0.5">
          {(
            [
              ['list', '人物列表'],
              ['dedupe', '去重建议'],
            ] as const
          ).map(([t, label]) => (
            <button
              key={t}
              onClick={() => setTab(t)}
              className={clsx(
                'rounded-full px-3 py-1 text-xs transition',
                tab === t ? 'bg-amber-300 font-semibold text-stone-950' : 'text-stone-400 hover:text-stone-200',
              )}
            >
              {label}
              {t === 'dedupe' && dedupeData && dedupeData.groups.length > 0 && (
                <span className={clsx('ml-1 rounded-full px-1.5 text-[0.6rem]', tab === t ? 'bg-stone-950/20' : 'bg-amber-300/20 text-amber-200')}>
                  {dedupeData.groups.length}
                </span>
              )}
            </button>
          ))}
        </div>
        {!isReadOnly && tab === 'list' && (
          <button
            onClick={() => setShowCreate((v) => !v)}
            className="flex items-center gap-1.5 rounded-lg bg-amber-300 px-3 py-1.5 text-xs font-semibold text-stone-950 transition hover:bg-amber-200"
          >
            <Plus className="h-3.5 w-3.5" />
            新建人物
          </button>
        )}
        <button
          onClick={refreshAll}
          className="flex items-center gap-1.5 rounded-lg border border-white/10 px-3 py-1.5 text-xs text-stone-400 transition hover:border-white/25 hover:text-stone-200"
        >
          <RefreshCw className="h-3.5 w-3.5" />
          刷新
        </button>
      </div>

      {showCreate && !isReadOnly && (
        <CreatePersonForm
          token={token}
          caseId={caseId}
          onDone={() => {
            setShowCreate(false)
            bump()
          }}
        />
      )}

      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
        {loading && (
          <div className="flex items-center gap-2 py-10 text-sm text-stone-500">
            <Loader2 className="h-4 w-4 animate-spin text-amber-300" />
            加载人物…
          </div>
        )}

        {tab === 'list' && !loading && (
          <div className="mx-auto flex max-w-3xl flex-col gap-3">
            {persons.length === 0 && <CenteredHint text="案件暂无人物记录。" />}

            <div className="flex flex-col gap-1.5">
              {persons.map((p) => (
                <button
                  key={p.id}
                  onClick={() => setSelection({ kind: 'person', person: p })}
                  className="flex items-center gap-3 rounded-xl border border-white/10 bg-white/4 px-3.5 py-2.5 text-left transition hover:border-amber-300/30 hover:bg-amber-300/6"
                >
                  <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-amber-300/25 bg-amber-300/10 text-sm text-amber-200">
                    {p.name.slice(0, 1)}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm font-medium text-stone-100">{p.name}</span>
                    <span className="mt-0.5 block truncate text-xs text-stone-500">
                      {p.role_type}
                      {p.organization ? ` · ${p.organization}` : ''}
                      {p.phone ? ` · ${p.phone}` : ''}
                    </span>
                  </span>
                </button>
              ))}
            </div>

            {/* Relationships */}
            {relData && relData.relationships.length > 0 && (
              <div className="mt-2">
                <p className="pb-2 text-xs font-semibold uppercase tracking-wider text-stone-500">
                  人物关系（{relData.relationships.length}）
                </p>
                <div className="flex flex-col gap-1.5">
                  {relData.relationships.map((r) => (
                    <div
                      key={r.id}
                      className="group flex items-center gap-2 rounded-xl border border-white/10 bg-white/4 px-3.5 py-2 text-sm"
                    >
                      <span className="text-stone-200">{r.from_person_name}</span>
                      <span className="rounded-full border border-amber-300/25 bg-amber-300/8 px-2 py-0.5 text-[0.65rem] text-amber-200">
                        {r.rel_type}
                      </span>
                      <span className="text-stone-200">{r.to_person_name}</span>
                      {r.rel_detail && <span className="text-xs text-stone-500">· {r.rel_detail}</span>}
                      {!isReadOnly && (
                        <button
                          onClick={() => void removeRel(r.id)}
                          className="ml-auto shrink-0 text-stone-600 opacity-0 transition hover:text-red-300 group-hover:opacity-100"
                          title="删除关系"
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                        </button>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            )}

            {!isReadOnly && (
              <RelationForm persons={persons} onCreate={createRel} />
            )}
          </div>
        )}

        {tab === 'dedupe' && !loading && (
          <div className="mx-auto max-w-3xl">
            {!dedupeData || dedupeData.groups.length === 0 ? (
              <CenteredHint text="没有发现疑似重复的人物。系统会按姓名、电话、邮箱分组提示。" />
            ) : (
              <div className="flex flex-col gap-2">
                {dedupeData.groups.map((g) => (
                  <div key={g.key} className="rounded-xl border border-amber-300/20 bg-amber-950/15 p-3.5">
                    <p className="text-xs text-amber-200/90">{g.reason}</p>
                    <div className="mt-2 flex flex-wrap items-center gap-2">
                      {g.persons.map((p, i) => (
                        <span key={p.id} className="flex items-center gap-2">
                          <span className="flex items-center gap-1.5 rounded-lg border border-white/10 bg-stone-900 px-2.5 py-1.5 text-xs text-stone-200">
                            <Users className="h-3 w-3 text-stone-500" />
                            {p.name}
                            <span className="text-[0.6rem] text-stone-500">{p.roles.join('/')}</span>
                          </span>
                          {i < g.persons.length - 1 && <span className="text-stone-600">+</span>}
                        </span>
                      ))}
                    </div>
                    <div className="mt-3 flex gap-1.5">
                      <button
                        onClick={() =>
                          addApprovals([
                            {
                              id: `ap-merge-${Date.now()}-${g.persons[0].id.slice(0, 6)}`,
                              title: `合并到「${g.persons[0].name}」`,
                              summary: `${g.reason}。保留 ${g.persons[0].name}，合并其余 ${g.persons.length - 1} 人。`,
                              level: 'sensitive',
                              execute: async () => {
                                const target = g.persons[0].id
                                const results = await Promise.all(
                                  g.persons.slice(1).map((src) =>
                                    mergePerson(token, caseId, src.id, target),
                                  ),
                                )
                                return results
                              },
                            },
                          ])
                        }
                        disabled={isReadOnly}
                        className="flex items-center gap-1.5 rounded-lg bg-amber-300 px-3 py-1.5 text-xs font-semibold text-stone-950 transition hover:bg-amber-200 disabled:opacity-40"
                      >
                        <GitMerge className="h-3.5 w-3.5" />
                        合并到 {g.persons[0].name}
                      </button>
                      <span className="self-center text-[0.65rem] text-stone-600">
                        敏感操作，加入待确认队列后执行
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  )
}

async function mergePerson(token: string, caseId: string, source: string, target: string) {
  return mergePersons(token, caseId, { source_person_id: source, target_person_id: target })
}

function CreatePersonForm({
  token,
  caseId,
  onDone,
}: {
  token: string
  caseId: string
  onDone: () => void
}) {
  const { ok, error } = useToasts()
  const [name, setName] = useState('')
  const [phone, setPhone] = useState('')
  const [email, setEmail] = useState('')
  const [organization, setOrganization] = useState('')
  const [position, setPosition] = useState('')
  const [roleType, setRoleType] = useState('当事人')
  const [busy, setBusy] = useState(false)

  const submit = async () => {
    if (!name.trim()) {
      error('姓名必填')
      return
    }
    setBusy(true)
    try {
      await createPerson(token, caseId, {
        name: name.trim(),
        phone: phone.trim() || undefined,
        email: email.trim() || undefined,
        organization: organization.trim() || undefined,
        position: position.trim() || undefined,
        role_type: roleType.trim() || undefined,
      })
      ok('人物已创建')
      onDone()
    } catch (e) {
      error(e instanceof Error ? e.message : '创建失败')
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="shrink-0 border-b border-white/10 bg-white/3 px-4 py-3">
      <div className="mx-auto grid max-w-2xl grid-cols-1 gap-2 sm:grid-cols-2">
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="姓名 *"
          className="rounded-lg border border-white/10 bg-stone-900 px-3 py-2 text-sm outline-none focus:border-amber-300/50"
        />
        <input
          value={roleType}
          onChange={(e) => setRoleType(e.target.value)}
          placeholder="角色（当事人/证人/律师/法官…）"
          className="rounded-lg border border-white/10 bg-stone-900 px-3 py-2 text-sm outline-none focus:border-amber-300/50"
        />
        <input
          value={phone}
          onChange={(e) => setPhone(e.target.value)}
          placeholder="电话"
          className="rounded-lg border border-white/10 bg-stone-900 px-3 py-2 text-sm outline-none focus:border-amber-300/50"
        />
        <input
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          placeholder="邮箱"
          className="rounded-lg border border-white/10 bg-stone-900 px-3 py-2 text-sm outline-none focus:border-amber-300/50"
        />
        <input
          value={organization}
          onChange={(e) => setOrganization(e.target.value)}
          placeholder="机构"
          className="rounded-lg border border-white/10 bg-stone-900 px-3 py-2 text-sm outline-none focus:border-amber-300/50"
        />
        <input
          value={position}
          onChange={(e) => setPosition(e.target.value)}
          placeholder="职位"
          className="rounded-lg border border-white/10 bg-stone-900 px-3 py-2 text-sm outline-none focus:border-amber-300/50"
        />
        <div className="flex gap-2 sm:col-span-2">
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

function RelationForm({
  persons,
  onCreate,
}: {
  persons: CasePerson[]
  onCreate: (from: string, to: string, relType: string, relDetail?: string) => void
}) {
  const [from, setFrom] = useState('')
  const [to, setTo] = useState('')
  const [relType, setRelType] = useState('')
  const [relDetail, setRelDetail] = useState('')

  const options = persons.filter((p) => p.id !== from)

  return (
    <div className="mt-3 rounded-xl border border-white/10 bg-white/4 p-3.5">
      <p className="flex items-center gap-1.5 pb-2 text-xs font-semibold uppercase tracking-wider text-stone-500">
        <Link2 className="h-3.5 w-3.5" />
        新建人物关系
      </p>
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        <select
          value={from}
          onChange={(e) => setFrom(e.target.value)}
          className="rounded-lg border border-white/10 bg-stone-900 px-2.5 py-2 text-sm outline-none focus:border-amber-300/50"
        >
          <option value="">人物 A</option>
          {persons.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
        <select
          value={relType}
          onChange={(e) => setRelType(e.target.value)}
          className="rounded-lg border border-white/10 bg-stone-900 px-2.5 py-2 text-sm outline-none focus:border-amber-300/50"
        >
          <option value="">关系类型</option>
          {['同事', '上下级', '亲属', '交易对手', '代理', '其他'].map((r) => (
            <option key={r} value={r}>
              {r}
            </option>
          ))}
        </select>
        <select
          value={to}
          onChange={(e) => setTo(e.target.value)}
          className="rounded-lg border border-white/10 bg-stone-900 px-2.5 py-2 text-sm outline-none focus:border-amber-300/50"
        >
          <option value="">人物 B</option>
          {options.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
        <button
          onClick={() => {
            if (!from || !to || !relType) return
            onCreate(from, to, relType, relDetail.trim() || undefined)
            setFrom('')
            setTo('')
            setRelType('')
            setRelDetail('')
          }}
          disabled={!from || !to || !relType}
          className="rounded-lg bg-amber-300 px-3 py-2 text-xs font-semibold text-stone-950 transition hover:bg-amber-200 disabled:opacity-40"
        >
          创建关系
        </button>
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
