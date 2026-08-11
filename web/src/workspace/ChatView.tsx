import { useEffect, useRef, useState } from 'react'
import {
  Bot,
  FileUp,
  History,
  Loader2,
  Paperclip,
  Search,
  Send,
  Sparkles,
  Users,
} from 'lucide-react'
import { useWorkspace } from './Workbench'
import type { AgentMessage } from '../lib/api'

const SUGGESTIONS: ReadonlyArray<{ label: string; prompt: string; icon: typeof History }> = [
  { label: '列出时间轴节点', prompt: '列出这个案件的时间轴节点', icon: History },
  { label: '列出证据文件', prompt: '列出这个案件的证据文件', icon: Paperclip },
  { label: '人物去重建议', prompt: '人物去重', icon: Users },
  { label: '搜索材料', prompt: '搜索 补充协议', icon: Search },
]

export function ChatView({
  messages,
  onSend,
}: {
  messages: AgentMessage[]
  onSend: (text: string) => Promise<void>
}) {
  const { isReadOnly, onUploadRequested } = useWorkspace()
  const [draft, setDraft] = useState('')
  const [thinking, setThinking] = useState(false)
  const scrollRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: 'smooth' })
  }, [messages, thinking])

  const send = async (raw: string) => {
    const text = raw.trim()
    if (!text || thinking) return
    setDraft('')
    setThinking(true)
    try {
      await onSend(text)
    } finally {
      setThinking(false)
    }
  }

  return (
    <div className="flex h-full flex-col">
      {/* Message stream */}
      <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto px-5 py-5">
        {messages.length === 0 && !thinking ? (
          <EmptyState onPick={(p) => void send(p)} onUpload={onUploadRequested} readOnly={isReadOnly} />
        ) : (
          <div className="mx-auto flex max-w-3xl flex-col gap-4">
            {messages.map((m) => (
              <MessageBubble key={m.id} message={m} />
            ))}
            {thinking && (
              <div className="flex items-center gap-2 text-sm text-stone-500">
                <Loader2 className="h-4 w-4 animate-spin text-amber-300" />
                Agent 正在处理…
              </div>
            )}
          </div>
        )}
      </div>

      {/* Composer */}
      <Composer
        draft={draft}
        setDraft={setDraft}
        onSend={() => void send(draft)}
        thinking={thinking}
        readOnly={isReadOnly}
        onUpload={onUploadRequested}
      />
    </div>
  )
}

function EmptyState({
  onPick,
  onUpload,
  readOnly,
}: {
  onPick: (p: string) => void
  onUpload: () => void
  readOnly: boolean
}) {
  const { caseName } = useWorkspace()
  return (
    <div className="mx-auto flex h-full max-w-2xl flex-col items-center justify-center gap-6 text-center">
      <div className="flex h-14 w-14 items-center justify-center rounded-2xl border border-amber-200/20 bg-amber-300/10 text-amber-200">
        <Sparkles className="h-7 w-7" />
      </div>
      <div>
        <h2 className="font-serif text-2xl text-white">
          {caseName ? `案件「${caseName}」` : 'LegalGenie Agent'}
        </h2>
        <p className="mt-2 text-sm leading-6 text-stone-400">
          对话由服务端 Agent 处理，写操作会先进入右侧「待确认动作」。
          <br />
          会话与审批记录持久化，可审计回放。
        </p>
      </div>

      <div className="grid w-full grid-cols-1 gap-2 sm:grid-cols-2">
        {SUGGESTIONS.map((s) => {
          const Icon = s.icon
          return (
            <button
              key={s.label}
              onClick={() => onPick(s.prompt)}
              className="flex items-center gap-2.5 rounded-xl border border-white/10 bg-white/4 px-4 py-3 text-left text-sm text-stone-200 transition hover:border-amber-300/30 hover:bg-amber-300/8"
            >
              <Icon className="h-4 w-4 shrink-0 text-amber-200" />
              {s.label}
            </button>
          )
        })}
      </div>

      {!readOnly && (
        <button
          onClick={onUpload}
          className="flex items-center gap-2 rounded-full border border-white/10 px-5 py-2.5 text-sm text-stone-300 transition hover:border-amber-300/40 hover:text-amber-200"
        >
          <FileUp className="h-4 w-4" />
          上传证据文件
        </button>
      )}
    </div>
  )
}

function MessageBubble({ message }: { message: AgentMessage }) {
  if (message.role === 'user') {
    return (
      <div className="flex justify-end">
        <div className="max-w-[80%] rounded-2xl rounded-br-md bg-amber-300/95 px-4 py-2.5 text-sm leading-6 text-stone-950">
          {message.content}
        </div>
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2 text-xs text-stone-500">
        <span className="flex h-5 w-5 items-center justify-center rounded-full bg-amber-300/15 text-amber-200">
          <Bot className="h-3 w-3" />
        </span>
        Agent
      </div>
      <div className="whitespace-pre-line text-sm leading-6 text-stone-200">{message.content}</div>
    </div>
  )
}

function Composer({
  draft,
  setDraft,
  onSend,
  thinking,
  readOnly,
  onUpload,
}: {
  draft: string
  setDraft: (s: string) => void
  onSend: () => void
  thinking: boolean
  readOnly: boolean
  onUpload: () => void
}) {
  return (
    <div className="shrink-0 border-t border-white/10 bg-stone-950/80 px-4 py-3">
      <div className="mx-auto flex max-w-3xl items-end gap-2">
        {!readOnly && (
          <button
            onClick={onUpload}
            title="上传证据文件"
            className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-white/10 text-stone-400 transition hover:border-amber-300/40 hover:text-amber-200"
          >
            <Paperclip className="h-4.5 w-4.5" />
          </button>
        )}
        <div className="min-w-0 flex-1 rounded-xl border border-white/10 bg-stone-900 px-3.5 py-2 transition focus-within:border-amber-300/40">
          <input
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
                e.preventDefault()
                onSend()
              }
            }}
            placeholder={
              readOnly
                ? '只读模式：可查询，写操作不可用'
                : '告诉 Agent 你想做什么，例如「新建节点：签约 2024-01-12」…'
            }
            disabled={thinking}
            className="w-full bg-transparent text-sm text-stone-100 outline-none placeholder:text-stone-600"
          />
          <p className="mt-0.5 hidden text-[0.65rem] text-stone-700 sm:block">
            支持：列出节点/证据/人物 · 搜索 · 人物去重 · 新建节点（标题+日期）· 导出
          </p>
        </div>
        <button
          onClick={onSend}
          disabled={thinking || !draft.trim()}
          className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-amber-300 text-stone-950 transition hover:bg-amber-200 disabled:cursor-not-allowed disabled:opacity-40"
          title="发送"
        >
          {thinking ? <Loader2 className="h-4.5 w-4.5 animate-spin" /> : <Send className="h-4.5 w-4.5" />}
        </button>
      </div>
    </div>
  )
}
