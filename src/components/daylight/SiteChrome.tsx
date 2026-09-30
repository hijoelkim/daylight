import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

function NavLink({ to, children }: { to: "/setup" | "/privacy"; children: string }) {
  return (
    <Link
      to={to}
      activeOptions={{ exact: true }}
      className="inline-flex min-h-11 items-center px-2 text-sm text-muted hover:text-fg data-[status=active]:text-fg"
    >
      {children}
    </Link>
  );
}

export function SiteChrome({ children }: { children: ReactNode }) {
  return (
    <div className="min-h-dvh bg-bg text-fg">
      <div className="mx-auto flex min-h-dvh max-w-5xl flex-col px-5 sm:px-8">
        <header className="flex items-center justify-between gap-4 py-4">
          <Link to="/" className="inline-flex min-h-11 items-center gap-3">
            <img src="/__daylight/mark.svg" alt="" width={32} height={32} className="size-8" />
            <span className="font-display text-xl tracking-widest">Daylight</span>
          </Link>
          <nav className="flex items-center" aria-label="Site">
            <NavLink to="/setup">Setup</NavLink>
            <NavLink to="/privacy">Privacy</NavLink>
          </nav>
        </header>
        <div className="flex-1">{children}</div>
        <footer className="mt-16 border-t border-border py-8">
          <p className="flex items-center gap-3 text-sm">
            <Link to="/setup" className="inline-flex min-h-11 items-center text-muted hover:text-fg">
              Setup
            </Link>
            <span aria-hidden="true" className="text-subtle">
              ·
            </span>
            <Link to="/privacy" className="inline-flex min-h-11 items-center text-muted hover:text-fg">
              Privacy
            </Link>
          </p>
          <p className="font-mono text-xs leading-relaxed text-muted">
            Data stays in %LOCALAPPDATA%\Daylight\ · no account · no cloud
          </p>
        </footer>
      </div>
    </div>
  );
}
