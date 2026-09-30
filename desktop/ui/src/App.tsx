import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useState } from "react";
import { AppList, type LiveApp } from "./components/AppList";
import { DayMeter } from "./components/DayMeter";
import { ReminderEditor, type ReminderRow } from "./components/ReminderEditor";
import { SettingsPanel, type LiveSettings } from "./components/SettingsPanel";
import { SunArc } from "./components/SunArc";
import { WeekChart, type WeekDay } from "./components/WeekChart";
import { formatMs } from "./format";

type Phase = "loading" | "consent" | "ready" | "unreachable";
type Tab = "Week" | "Reminders" | "Settings";

type Today = {
  active_ms: number;
  idle_ms: number;
  locked_ms: number;
  apps: LiveApp[];
  current: { app_key: string; product_name: string } | null;
  paused?: boolean;
};

type Sun = {
  sunrise_clock?: string;
  solar_noon_clock?: string;
  sunset_clock?: string;
  daylight_ms?: number | null;
  now_fraction_along_arc?: number | null;
  polar?: "up" | "down" | null;
};

const CONSENT =
  "Daylight records which app is in the foreground. Data stays in %LOCALAPPDATA%\\Daylight. No cloud. No keystrokes. No screenshots.";

const emptySettings: LiveSettings = {
  idle_threshold_s: "60",
  retain_days: "90",
  record_titles: "0",
  start_with_windows: "0",
  city: "",
  lat: "-33.8688",
  lon: "151.2093",
};

export function App() {
  const [phase, setPhase] = useState<Phase>("loading");
  const [busy, setBusy] = useState(false);
  const [tab, setTab] = useState<Tab>("Week");
  const [today, setToday] = useState<Today | null>(null);
  const [sun, setSun] = useState<Sun | null>(null);
  const [week, setWeek] = useState<WeekDay[]>([]);
  const [reminders, setReminders] = useState<ReminderRow[]>([]);
  const [settings, setSettings] = useState<LiveSettings>(emptySettings);
  const [note, setNote] = useState("");

  const loadLive = useCallback(async () => {
    const [nextToday, nextSun] = await Promise.all([
      invoke<Today>("get_today"),
      invoke<Sun>("get_sun"),
    ]);
    setToday(nextToday);
    setSun(nextSun);
    const rows = await invoke<Array<{ date: string; active_ms: number | null }>>("get_week");
    setWeek(
      (rows ?? []).map((row) => ({
        day: weekday(row.date),
        hours: (row.active_ms ?? 0) / 3_600_000,
      })),
    );
  }, []);

  useEffect(() => {
    let cancelled = false;
    invoke<boolean>("consent_accepted")
      .then((accepted) => {
        if (!cancelled) setPhase(accepted ? "ready" : "consent");
      })
      .catch(() => {
        if (!cancelled) setPhase("unreachable");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (phase !== "ready") return;
    let stop = false;
    async function tick() {
      const visible = await getCurrentWindow().isVisible().catch(() => true);
      if (stop || !visible) return;
      await loadLive().catch(() => undefined);
    }
    void tick();
    const id = window.setInterval(() => void tick(), 15_000);
    return () => {
      stop = true;
      window.clearInterval(id);
    };
  }, [phase, loadLive]);

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") void getCurrentWindow().hide();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (phase !== "ready") return;
    void invoke<Record<string, string>>("get_settings").then((rows) => {
      setSettings({ ...emptySettings, ...rows });
    });
    void invoke<ReminderRow[]>("list_reminders").then(setReminders).catch(() => undefined);
  }, [phase]);

  async function accept() {
    setBusy(true);
    try {
      await invoke("accept_consent");
      setPhase("ready");
    } finally {
      setBusy(false);
    }
  }

  async function saveSettings(patch: Partial<LiveSettings>) {
    const next = { ...settings, ...patch };
    setSettings(next);
    await invoke("set_settings", { settings: patch });
    if (patch.city || patch.lat || patch.lon) await loadLive();
  }

  const empty = (today?.active_ms ?? 0) === 0;
  const polar = sun?.polar === "up" || sun?.polar === "down";
  const pct =
    sun?.daylight_ms && today
      ? Math.round((today.active_ms / sun.daylight_ms) * 100)
      : 0;
  const noonT = 0.5;
  const nowT = sun?.now_fraction_along_arc ?? 0;

  return (
    <main className="min-h-dvh bg-bg px-6 py-8 text-fg">
      {phase === "loading" ? <p className="font-mono text-sm text-muted">Daylight</p> : null}
      {phase === "unreachable" ? <p className="text-pretty leading-relaxed text-muted">Could not reach the shell.</p> : null}
      {phase === "consent" ? (
        <div className="mx-auto w-full max-w-xl">
          <img src="/mark.svg" alt="" width={32} height={32} className="size-8" />
          <h1 className="mt-8 font-display text-5xl font-medium leading-none tracking-wide text-balance">Before it watches</h1>
          <p className="mt-6 max-w-md text-pretty leading-relaxed text-muted">{CONSENT}</p>
          <div className="mt-8 flex flex-wrap items-center gap-4">
            <button type="button" disabled={busy} onClick={() => void accept()} className="inline-flex min-h-11 items-center border border-accent bg-bg-elevated px-5 font-display text-xl tracking-wide text-fg disabled:opacity-50">
              Accept and start
            </button>
            <button type="button" disabled={busy} onClick={() => void invoke("decline_consent")} className="inline-flex min-h-11 items-center px-3 text-sm text-muted">
              Decline and quit
            </button>
          </div>
        </div>
      ) : null}
      {phase === "ready" ? (
        <div className="mx-auto w-full max-w-4xl pb-8">
          <h1 className="font-display text-4xl font-medium tracking-wide sm:text-5xl">Today</h1>
          <section className="mt-8" aria-label="Day arc">
            <SunArc rise={sun?.sunrise_clock || "--:--"} set={sun?.sunset_clock || "--:--"} noonT={noonT} nowT={polar ? 0 : nowT} />
            <DayMeter
              screen={`${formatMs(today?.active_ms ?? 0)} on screen`}
              daylightPct={pct}
              secondary={`${formatMs(today?.idle_ms ?? 0)} idle · ${formatMs(today?.locked_ms ?? 0)} locked`}
              empty={empty}
              polar={polar}
            />
          </section>
          <AppList dateLabel={todayLabel()} apps={today?.apps ?? []} current={today?.current ?? null} />
          <section className="mt-12" aria-label="More">
            <div role="tablist" aria-label="Today details" className="flex gap-2 border-b border-border">
              {(["Week", "Reminders", "Settings"] as const).map((name) => (
                <button key={name} type="button" role="tab" aria-selected={tab === name} onClick={() => setTab(name)} className={`inline-flex min-h-11 items-center border-b-2 px-3 font-display text-xl tracking-wide ${tab === name ? "border-accent text-fg" : "border-transparent text-muted"}`}>
                  {name}
                </button>
              ))}
            </div>
            <div className="mt-6" role="tabpanel">
              {tab === "Week" ? <WeekChart days={week} /> : null}
              {tab === "Reminders" ? (
                <ReminderEditor
                  reminders={reminders}
                  onSave={(reminder) => {
                    void invoke("save_reminder", { reminder }).then(() => invoke<ReminderRow[]>("list_reminders").then(setReminders));
                  }}
                  onDelete={(id) => {
                    void invoke("delete_reminder", { id }).then(() => setReminders((rows) => rows.filter((row) => row.id !== id)));
                  }}
                  onTest={(id) => {
                    void invoke("test_reminder", { id }).then(() => setNote("Test sent."));
                  }}
                  onAdd={() => {
                    void invoke("save_reminder", {
                      reminder: { kind: "interval", title: "", body: "", interval_min: 60, time_local: "09:00", after_screen_min: 30, enabled: 1 },
                    }).then(() => invoke<ReminderRow[]>("list_reminders").then(setReminders));
                  }}
                />
              ) : null}
              {tab === "Settings" ? (
                <SettingsPanel
                  settings={settings}
                  paused={Boolean(today?.paused)}
                  onChange={(patch) => void saveSettings(patch)}
                  onExport={() => {
                    void invoke<string>("export_data").then((path) => setNote(path));
                  }}
                  onWipe={() => {
                    void invoke("wipe_data").then(() => loadLive()).then(() => setNote("History wiped."));
                  }}
                  onPause={() => {
                    void invoke(today?.paused ? "resume" : "pause").then(() => loadLive());
                  }}
                  onUpdate={() => {
                    setNote("Checking for updates…");
                    void invoke<{ state: string; version: string; latest?: string }>("apply_update")
                      .then((result) => {
                        if (result.state === "current") setNote(`Up to date. ${result.version}.`);
                        else setNote(`Installing ${result.latest}. Daylight will close and reopen.`);
                      })
                      .catch((err: unknown) => setNote(err instanceof Error ? err.message : String(err)));
                  }}
                />
              ) : null}
            </div>
          </section>
          {note ? <p className="mt-6 font-mono text-xs text-muted">{note}</p> : null}
        </div>
      ) : null}
    </main>
  );
}

function weekday(date: string): string {
  const parsed = new Date(`${date}T12:00:00`);
  if (Number.isNaN(parsed.getTime())) return date;
  return parsed.toLocaleDateString("en-GB", { weekday: "short" });
}

function todayLabel(): string {
  return new Date().toLocaleDateString("en-GB", { weekday: "short", day: "numeric", month: "short" });
}
