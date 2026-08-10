import clsx from 'clsx'
import {
  Bot,
  FileSearch,
  Files,
  FolderKanban,
  History,
  Paperclip,
  Users,
} from 'lucide-react'

export type WorkspaceView = 'chat' | 'timeline' | 'evidence' | 'persons' | 'search' | 'exports' | 'logs'

const ITEMS: ReadonlyArray<{
  id: WorkspaceView
  label: string
  icon: typeof Bot
}> = [
  { id: 'chat', label: '对话', icon: Bot },
  { id: 'timeline', label: '时间轴', icon: History },
  { id: 'evidence', label: '证据', icon: Paperclip },
  { id: 'persons', label: '人物', icon: Users },
  { id: 'search', label: '搜索', icon: FileSearch },
  { id: 'exports', label: '导出', icon: Files },
  { id: 'logs', label: '日志', icon: FolderKanban },
]

export function LeftRail({
  view,
  onView,
  approvalCount,
}: {
  view: WorkspaceView
  onView: (v: WorkspaceView) => void
  approvalCount: number
}) {
  return (
    <nav className="flex w-[4.25rem] shrink-0 flex-col items-center gap-1 border-r border-white/10 bg-stone-950/70 py-3">
      {ITEMS.map((item) => {
        const Icon = item.icon
        const active = view === item.id
        return (
          <button
            key={item.id}
            onClick={() => onView(item.id)}
            title={item.label}
            className={clsx(
              'relative flex w-12 flex-col items-center gap-1 rounded-xl py-2.5 text-[0.6rem] transition',
              active
                ? 'bg-amber-300/12 text-amber-200'
                : 'text-stone-500 hover:bg-white/6 hover:text-stone-200',
            )}
          >
            <Icon className="h-4.5 w-4.5" />
            {item.label}
            {item.id === 'chat' && approvalCount > 0 && (
              <span className="absolute right-1 top-1 flex h-4 min-w-4 items-center justify-center rounded-full bg-amber-300 px-1 text-[0.6rem] font-bold text-stone-950">
                {approvalCount}
              </span>
            )}
          </button>
        )
      })}
    </nav>
  )
}
