import { ArrowRight } from 'lucide-react'
import { PageShell } from './layout'

export function OverviewPage() {
  return (
    <PageShell
      currentPath="/overview.html"
      navItems={[]}
      utilityHref="/index.html"
      utilityLabel="Workspace"
    >
      <section className="py-10">
        <div className="max-w-4xl rounded-[28px] border border-white/10 bg-white/5 p-7 sm:p-9">
          <p className="text-[0.72rem] font-semibold uppercase tracking-[0.28em] text-stone-500">
            Overview
          </p>
          <h2 className="mt-4 font-serif text-4xl leading-tight text-white sm:text-5xl">
            只留必要说明。
          </h2>
          <div className="mt-8 grid gap-6 md:grid-cols-2">
            <div>
              <h3 className="text-lg font-semibold text-white">它是什么</h3>
              <ul className="mt-3 space-y-3 text-sm leading-7 text-stone-300">
                <li>一个法律 AI 工作台，不是通用聊天工具。</li>
                <li>围绕案件上下文组织时间轴、证据、人物、日志和导出。</li>
                <li>让 AI 参与执行，但把高风险动作留给人工确认。</li>
              </ul>
            </div>
            <div>
              <h3 className="text-lg font-semibold text-white">给谁用</h3>
              <ul className="mt-3 space-y-3 text-sm leading-7 text-stone-300">
                <li>争议解决团队</li>
                <li>法务运营团队</li>
                <li>调查与合规团队</li>
              </ul>
            </div>
            <div>
              <h3 className="text-lg font-semibold text-white">为什么可信</h3>
              <ul className="mt-3 space-y-3 text-sm leading-7 text-stone-300">
                <li>案件角色权限</li>
                <li>操作日志与导出记录</li>
                <li>本地部署路径</li>
              </ul>
            </div>
            <div>
              <h3 className="text-lg font-semibold text-white">怎么落地</h3>
              <ul className="mt-3 space-y-3 text-sm leading-7 text-stone-300">
                <li>Vite + Bun 前端站点</li>
                <li>本地运行与 Docker 部署</li>
                <li>可继续按团队流程二次定制</li>
              </ul>
            </div>
          </div>
          <div className="mt-8">
            <a
              href="/index.html"
              className="inline-flex items-center gap-2 rounded-full bg-amber-300 px-5 py-3 text-sm font-semibold text-stone-950 transition hover:bg-amber-200"
            >
              Back to workspace
              <ArrowRight className="h-4 w-4" />
            </a>
          </div>
        </div>
      </section>
    </PageShell>
  )
}
