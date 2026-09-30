import { createFileRoute } from "@tanstack/react-router";
import { SiteChrome } from "@/components/daylight/SiteChrome";

export const Route = createFileRoute("/privacy")({
  head: () => ({ meta: [{ title: "Your machine keeps the data" }] }),
  component: Privacy,
});

const recorded = [
  "Foreground app name and how long it was in front",
  "A website's name, once that site has been in front for five minutes",
  "Idle and lock gaps, so away time is not screen time",
  "Sunrise/sunset from the location you set",
  "Reminders you wrote",
] as const;

const notCollected = [
  "Keystrokes, clipboard, screenshots, or the path after a site name",
  "Sites you only open briefly. Those stay inside the browser",
  "Window titles, unless you turn that on",
  "Anything to a server",
] as const;

function Privacy() {
  return (
    <SiteChrome>
      <main className="max-w-xl">
        <h1 className="mt-10 font-display text-4xl font-medium leading-tight tracking-wide text-balance sm:mt-16 sm:text-6xl">
          Your machine keeps the data
        </h1>
        <section className="mt-10" aria-labelledby="recorded-heading">
          <h2 id="recorded-heading" className="font-mono text-sm text-accent">
            Recorded
          </h2>
          <ul className="mt-4 space-y-3">
            {recorded.map((item) => (
              <li key={item} className="border-l border-border pl-4 text-pretty leading-relaxed">
                {item}
              </li>
            ))}
          </ul>
        </section>
        <section className="mt-10" aria-labelledby="not-collected-heading">
          <h2 id="not-collected-heading" className="font-mono text-sm text-accent">
            Not collected
          </h2>
          <ul className="mt-4 space-y-3">
            {notCollected.map((item) => (
              <li key={item} className="border-l border-border pl-4 text-pretty leading-relaxed text-muted">
                {item}
              </li>
            ))}
          </ul>
        </section>
        <p className="mt-10 font-mono text-sm leading-relaxed text-muted">
          Where: %LOCALAPPDATA%\Daylight\
        </p>
        <p className="mt-3 text-pretty leading-relaxed">You can export or wipe from Settings.</p>
      </main>
    </SiteChrome>
  );
}
