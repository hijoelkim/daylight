import { createFileRoute } from "@tanstack/react-router";
import { DownloadButton } from "@/components/daylight/DownloadButton";
import { SiteChrome } from "@/components/daylight/SiteChrome";
import { SmartScreenNote } from "@/components/daylight/SmartScreenNote";

export const Route = createFileRoute("/setup")({
  head: () => ({ meta: [{ title: "If something gets in the way" }] }),
  component: Setup,
});

const blockers = [
  {
    title: "Windows doesn’t recognize the app",
    lines: ["More info → Run anyway. Unsigned build. Expected."],
  },
  {
    title: "The window vanished",
    lines: [
      "It is in the tray, near the clock. Left click opens it. Right click quits.",
      "Closing the window does not quit.",
    ],
  },
  {
    title: "It asks for location",
    lines: [
      "Used only to compute sunrise and sunset on this PC. You can type a city instead.",
      "Default guess: Sydney.",
    ],
  },
  {
    title: "Nothing is being recorded",
    lines: [
      "First launch shows a consent card. Tracking starts after you accept.",
      'Check the tray menu says "Recording".',
    ],
  },
  {
    title: "Toasts never appear",
    lines: [
      "Windows hides toasts for unpackaged apps unless Daylight is pinned to Start.",
      "The installer creates the Start Menu shortcut. Do not delete it.",
      "Also check Settings → System → Notifications.",
    ],
  },
  {
    title: "WebView2 missing",
    lines: [
      "Windows 10/11 usually have it. If the window is blank, install Microsoft Edge WebView2 Runtime (Evergreen).",
    ],
  },
  {
    title: "It didn’t start with Windows",
    lines: ["Open Daylight → Settings → Start with Windows."],
  },
] as const;

function Setup() {
  return (
    <SiteChrome>
      <main className="max-w-xl">
        <h1 className="mt-10 font-display text-4xl font-medium leading-tight tracking-wide text-balance sm:mt-16 sm:text-6xl">
          If something gets in the way
        </h1>
        <div className="mt-8 flex flex-col items-start gap-3">
          <DownloadButton />
          <SmartScreenNote />
        </div>
        <div className="mt-12">
          {blockers.map((item) => (
            <section key={item.title} className="border-t border-border py-6">
              <h2 className="font-display text-2xl tracking-wide">{item.title}</h2>
              <div className="mt-2 space-y-2 text-pretty leading-relaxed text-muted">
                {item.lines.map((line) => (
                  <p key={line}>{line}</p>
                ))}
              </div>
            </section>
          ))}
        </div>
      </main>
    </SiteChrome>
  );
}
