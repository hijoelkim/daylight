import { createFileRoute, Link } from "@tanstack/react-router";
import { AlarmClock } from "lucide-react";
import type { ReactNode } from "react";
import { DownloadButton } from "@/components/daylight/DownloadButton";
import { SiteChrome } from "@/components/daylight/SiteChrome";
import { SmartScreenNote } from "@/components/daylight/SmartScreenNote";

export const Route = createFileRoute("/")({
  head: () => ({ meta: [{ title: "more day, less screen" }] }),
  component: Home,
});

const steps = [
  {
    kicker: "1 · PC",
    lines: [
      "Download and run it. Allow Windows when it asks.",
      "A window opens. After this first time, Daylight lives in the tray.",
    ],
  },
  {
    kicker: "2 · Leave it",
    lines: [
      "It records which app is in front. Idle and a locked screen do not count.",
      "Nothing leaves this computer.",
    ],
  },
  {
    kicker: "3 · Open it",
    lines: [
      "Click the tray icon. Sunrise and sunset sit on an arc.",
      "Under that: hours on screen, a list of apps, a week graph, and your reminders.",
    ],
  },
] as const;

const sampleApps = [
  { name: "Chrome", width: "82%" },
  { name: "Cursor", width: "54%" },
  { name: "Explorer", width: "28%" },
] as const;

function Home() {
  return (
    <SiteChrome>
      <main>
        <h1 className="mt-10 max-w-3xl font-display text-5xl font-medium leading-none tracking-wide text-balance sm:mt-16 sm:text-7xl">
          more day, less screen
        </h1>
        <div className="mt-8 max-w-xl space-y-3 text-pretty leading-relaxed text-muted">
          <p>Daylight sits in the Windows tray and watches which app is in front.</p>
          <p>Open it and you get sunrise, sunset, hours on screen, and the apps that ate them.</p>
          <p>Reminders ping when you ask — every hour, at sunset, after too long on one thing.</p>
        </div>
        <div className="mt-8 flex flex-col items-start gap-3">
          <DownloadButton />
          <SmartScreenNote />
        </div>

        <ol className="mt-16 max-w-xl space-y-10">
          {steps.map((step) => (
            <li key={step.kicker}>
              <p className="font-mono text-sm text-accent">{step.kicker}</p>
              <div className="mt-2 space-y-2 text-pretty leading-relaxed">
                {step.lines.map((line) => (
                  <p key={line}>{line}</p>
                ))}
              </div>
            </li>
          ))}
        </ol>

        <section className="mt-20" aria-labelledby="screens-heading">
          <div className="flex flex-wrap items-end justify-between gap-4">
            <h2
              id="screens-heading"
              className="font-display text-3xl font-medium tracking-wide text-balance sm:text-4xl"
            >
              See more — the screens
            </h2>
            <Link
              to="/app"
              className="inline-flex min-h-11 items-center font-display text-xl tracking-wide text-accent"
            >
              Open the demo
            </Link>
          </div>
          <div className="mt-8 grid gap-4 md:grid-cols-3">
            <DayArcFrame />
            <AppListFrame />
            <ReminderFrame />
          </div>
        </section>
      </main>
    </SiteChrome>
  );
}

function Frame({ kicker, children }: { kicker: string; children: ReactNode }) {
  return (
    <article className="flex h-full flex-col border border-border bg-bg-elevated">
      <p className="border-b border-border px-4 py-2 font-mono text-xs text-muted">{kicker}</p>
      {children}
    </article>
  );
}

function DayArcFrame() {
  return (
    <Frame kicker="Day">
      <div className="relative aspect-video bg-horizon-sky">
        <div className="absolute inset-x-0 bottom-0 h-1/4 bg-horizon-ground" />
        <svg
          viewBox="0 0 320 180"
          className="relative h-full w-full font-mono"
          role="img"
          aria-label="Sunrise, now, and sunset"
        >
          <path
            d="M32 132 Q160 28 288 132"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            className="text-accent"
          />
          <circle cx="32" cy="132" r="3" className="fill-accent" />
          <circle cx="104" cy="90" r="4" className="fill-fg" />
          <circle cx="288" cy="132" r="3" className="fill-muted" />
          <text x="32" y="158" className="fill-muted" fontSize="11">
            sunrise
          </text>
          <text x="104" y="78" textAnchor="middle" className="fill-fg" fontSize="11">
            now
          </text>
          <text x="288" y="158" textAnchor="end" className="fill-muted" fontSize="11">
            set
          </text>
        </svg>
      </div>
      <p className="border-t border-border px-4 py-3 font-mono text-xs leading-relaxed text-muted">
        3h 12m on screen · 28% of daylight
      </p>
    </Frame>
  );
}

function AppListFrame() {
  return (
    <Frame kicker="Apps">
      <ul className="flex flex-1 flex-col justify-center gap-4 px-4 py-5">
        {sampleApps.map((app) => (
          <li key={app.name}>
            <p className="text-sm">{app.name}</p>
            <div className="mt-2 h-1.5 bg-bg-subtle" aria-hidden="true">
              <div className="h-full bg-accent" style={{ width: app.width }} />
            </div>
          </li>
        ))}
      </ul>
    </Frame>
  );
}

function ReminderFrame() {
  return (
    <Frame kicker="Reminder">
      <div className="flex flex-1 items-center gap-3 px-4 py-5">
        <AlarmClock className="size-4 shrink-0 text-accent" aria-hidden="true" />
        <p className="font-mono text-sm leading-relaxed">Every 60 minutes · stand up</p>
      </div>
    </Frame>
  );
}
