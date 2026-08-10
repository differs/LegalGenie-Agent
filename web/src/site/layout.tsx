import type { PropsWithChildren } from 'react'
import clsx from 'clsx'
import { ArrowRight, Scale, ShieldCheck } from 'lucide-react'

type PageShellProps = PropsWithChildren<{
  currentPath: string
  navItems: ReadonlyArray<{ href: string; label: string }>
  utilityHref: string
  utilityLabel: string
}>

export function PageShell({
  currentPath,
  navItems,
  utilityHref,
  utilityLabel,
  children,
}: PageShellProps) {
  return (
    <div className="min-h-screen bg-stone-950 text-stone-100">
      <div className="absolute inset-x-0 top-0 h-[24rem] bg-[radial-gradient(circle_at_top,rgba(208,170,99,0.14),transparent_52%),linear-gradient(180deg,rgba(16,19,22,0.96),rgba(16,19,22,0))]" />

      <div className="relative z-10 mx-auto flex min-h-screen max-w-7xl flex-col px-4 pb-20 pt-5 sm:px-6 lg:px-8">
        <header className="border-b border-white/10 py-4">
          <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
            <a href="/index.html" className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-2xl border border-amber-200/20 bg-amber-300/10 text-amber-200">
                <Scale className="h-5 w-5" />
              </div>
              <div>
                <p className="text-[0.64rem] font-semibold uppercase tracking-[0.28em] text-stone-500">
                  Legal AI Workspace
                </p>
                <h1 className="font-serif text-lg text-white">LegalGenie Agent</h1>
              </div>
            </a>

            <nav className="flex flex-wrap items-center gap-2">
              {navItems.map((item) => {
                const active = currentPath === item.href

                return (
                  <a
                    key={item.href}
                    href={item.href}
                    className={clsx(
                      'rounded-full px-4 py-2 text-sm transition',
                      active
                        ? 'bg-white/10 text-white'
                        : 'text-stone-400 hover:bg-white/6 hover:text-stone-200',
                    )}
                  >
                    {item.label}
                  </a>
                )
              })}
            </nav>

            <div className="flex flex-wrap items-center gap-3">
              <span className="inline-flex items-center gap-2 rounded-full border border-emerald-400/20 bg-emerald-400/8 px-4 py-2 text-sm text-emerald-100">
                <ShieldCheck className="h-4 w-4" />
                Open source, self-hostable
              </span>
              <a
                href={utilityHref}
                className="inline-flex items-center gap-2 rounded-full bg-amber-300 px-4 py-2 text-sm font-semibold text-stone-950 transition hover:bg-amber-200"
              >
                {utilityLabel}
                <ArrowRight className="h-4 w-4" />
              </a>
            </div>
          </div>
        </header>

        <main className="mt-6 flex-1">{children}</main>

        <footer className="mt-16 border-t border-white/10 pt-8 text-sm text-stone-500">
          <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
            <p>LegalGenie Agent workspace</p>
            <div className="flex flex-wrap gap-4">
              <a href="/index.html" className="hover:text-stone-100">
                Workspace
              </a>
              <a href="/overview.html" className="hover:text-stone-100">
                Overview
              </a>
            </div>
          </div>
        </footer>
      </div>
    </div>
  )
}

export function CTA({
  title,
  body,
  href,
  label,
}: {
  title: string
  body: string
  href: string
  label: string
}) {
  return (
    <section className="mt-16 rounded-[28px] border border-white/10 bg-white/5 p-8 sm:p-10">
      <div className="flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
        <div className="max-w-3xl">
          <h3 className="mt-3 font-serif text-3xl text-white sm:text-4xl">{title}</h3>
          <p className="mt-4 text-base leading-8 text-stone-300">{body}</p>
        </div>
        <a
          href={href}
          className="inline-flex items-center justify-center gap-2 rounded-full bg-amber-300 px-5 py-3 text-sm font-semibold text-stone-950 transition hover:bg-amber-200"
        >
          {label}
          <ArrowRight className="h-4 w-4" />
        </a>
      </div>
    </section>
  )
}
