import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useState } from "react";
import { AppList, type LiveApp } from "./components/AppList";
import { DayMeter } from "./components/DayMeter";
import { ReminderEditor, type ReminderRow } from "./components/ReminderEditor";
import { ScreenBattery } from "./components/ScreenBattery";
import { SettingsPanel, type LiveSettings } from "./components/SettingsPanel";
import { SunArc, type ArcMark } from "./components/SunArc";
import { WeekChart, type WeekDay } from "./components/WeekChart";
import { formatMs } from "./format";

type Phase = "loading" | "consent" | "ready" | "unreachable";
type Tab = "Week" | "Reminders" | "Settings";

type Span = { color: string; start: number; end: number };

type Today = {
  active_ms: number;
  idle_ms: number;
  locked_ms: number;
  apps: LiveApp[];
  spans?: Span[];
  current: { app_key: string; product_name: string } | null;
  paused?: boolean;
  hardcore_streak?: number;
  hardcore_broken?: boolean;
  zero_prompt?: boolean;
};

type Sun = {
  sunrise?: number | null;
  sunset?: number | null;
  solar_noon?: number | null;
  day_start?: number | null;
  sunrise_clock?: string;
  solar_noon_clock?: string;
  sunset_clock?: string;
  daylight_ms?: number | null;
  now_fraction_along_arc?: number | null;
  polar?: "up" | "down" | null;
};

const CONSENT =
  "Daylight records the foreground app on this PC, in %LOCALAPPDATA%\\Daylight. A site in front for five minutes is stored as a hostname only. Window titles stay off until you turn them on. It does not record keys, clipboard, or screenshots. Hardcore download mode only notes that a key was pressed, not which one. Screen time is not uploaded. A confirmed update is downloaded from GitHub after its hash matches.";

const emptySettings: LiveSettings = {
  idle_threshold_s: "60",
  retain_days: "90",
  record_titles: "0",
  start_with_windows: "1",
  city: "",
  lat: "-33.8688",
  lon: "151.2093",
  screen_budget_min: "0",
  hardcore: "0",
  auto_dim: "0",
  dim_by: "50",
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
  const [zeroOpen, setZeroOpen] = useState(false);
  const budgetMin = Number(settings.screen_budget_min) || 0;
  const batteryEmpty = budgetMin > 0 && (today?.active_ms ?? 0) >= budgetMin * 60_000;

  useEffect(() => {
    document.documentElement.style.setProperty("--color-bg", batteryEmpty ? "#2c1214" : "#08090c");
  }, [batteryEmpty]);

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
      if (event.key === "Escape" && !zeroOpen) void getCurrentWindow().hide();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [zeroOpen]);

  useEffect(() => {
    if (today?.zero_prompt) setZeroOpen(true);
    if (today && today.zero_prompt === false) setZeroOpen(false);
  }, [today]);

  useEffect(() => {
    let stop = false;
    let unlisten: (() => void) | undefined;
    void listen("zero-choice", () => {
      if (!stop) setZeroOpen(true);
    }).then((fn) => {
      if (stop) fn();
      else unlisten = fn;
    });
    return () => {
      stop = true;
      unlisten?.();
    };
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

  async function chooseZero(choice: "sleep" | "power" | "download") {
    setToday((current) => (current ? { ...current, zero_prompt: false } : current));
    setZeroOpen(false);
    try {
      await invoke("choose_zero", { choice });
    } catch (err) {
      setToday((current) => (current ? { ...current, zero_prompt: true } : current));
      setZeroOpen(true);
      setNote(err instanceof Error ? err.message : String(err));
    }
  }

  const empty = (today?.active_ms ?? 0) === 0;
  const polar = sun?.polar === "up" || sun?.polar === "down";
  const pct =
    sun?.daylight_ms && today
      ? Math.round((today.active_ms / sun.daylight_ms) * 100)
      : 0;
  const noonT = 0.5;
  const dayStart = sun?.day_start ?? 0;
  const dayMs = 86_400_000;
  const fraction = (ms: number) => (ms - dayStart) / dayMs;
  const marks: ArcMark[] =
    dayStart > 0
      ? (today?.spans ?? []).flatMap((span) => {
          const t0 = Math.max(0, fraction(span.start));
          const t1 = Math.min(1, fraction(span.end));
          return t1 > t0 ? [{ t0, t1, color: span.color }] : [];
        })
      : [];
  const riseT = sun?.sunrise != null && dayStart > 0 ? fraction(sun.sunrise) : null;
  const setT = sun?.sunset != null && dayStart > 0 ? fraction(sun.sunset) : null;
  const noonAlong = sun?.solar_noon != null && dayStart > 0 ? fraction(sun.solar_noon) : noonT;
  const nowAlong = dayStart > 0 ? Math.min(1, Math.max(0, fraction(Date.now()))) : 0;

  return (
    <main className="min-h-dvh bg-bg px-6 py-8 text-fg" style={batteryEmpty ? { background: "#2c1214" } : undefined}>
      {zeroOpen && phase === "ready" ? (
        <div className="fixed inset-0 z-20 flex items-center justify-center bg-bg/90 px-6">
          <div className="w-full max-w-md border border-border bg-bg-elevated p-6">
            <h2 className="font-display text-4xl tracking-wide">Max screen time</h2>
            <p className="mt-4 text-pretty leading-relaxed text-muted">
              The battery is at zero. The computer stays on until you choose.
            </p>
            <div className="mt-6 flex flex-col items-start gap-3">
              <button type="button" className="inline-flex min-h-11 items-center text-fg" onClick={() => void chooseZero("sleep")}>
                Sleep the computer
              </button>
              <button type="button" className="inline-flex min-h-11 items-center text-fg" onClick={() => void chooseZero("power")}>
                Power off
              </button>
              <button type="button" className="inline-flex min-h-11 items-center text-fg" onClick={() => void chooseZero("download")}>
                Download mode
              </button>
            </div>
          </div>
        </div>
      ) : null}
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
            <SunArc
              rise={sun?.sunrise_clock || "--:--"}
              set={sun?.sunset_clock || "--:--"}
              riseT={polar ? null : riseT}
              setT={polar ? null : setT}
              noonT={noonAlong}
              nowT={nowAlong}
              marks={marks}
            />
            <DayMeter
              screen={`${formatMs(today?.active_ms ?? 0)} on screen`}
              daylightPct={pct}
              secondary={`${formatMs(today?.idle_ms ?? 0)} idle · ${formatMs(today?.locked_ms ?? 0)} locked`}
              empty={empty}
              polar={polar}
            />
            <ScreenBattery usedMs={today?.active_ms ?? 0} budgetMin={Number(settings.screen_budget_min) || 0} />
            {settings.hardcore === "1" ? (
              <p className="mt-2 font-mono text-sm tabular-nums text-muted">
                {today?.hardcore_broken
                  ? "Hardcore streak reset"
                  : `Hardcore streak · ${today?.hardcore_streak ?? 0} ${(today?.hardcore_streak ?? 0) === 1 ? "day" : "days"}`}
              </p>
            ) : null}
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
              {tab === "Week" ? <WeekChart days={week} budgetMin={Number(settings.screen_budget_min) || 0} /> : null}
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
                    if (!window.confirm("Wipe history? Screen time on this computer will be deleted.")) return;
                    void invoke("wipe_data").then(() => loadLive()).then(() => setNote("History wiped."));
                  }}
                  onPause={() => {
                    void invoke(today?.paused ? "resume" : "pause").then(() => loadLive());
                  }}
                  onUpdate={() => {
                    setNote("Checking for updates…");
                    void invoke<{ state: string; version?: string; latest?: string; reason?: string }>("apply_update")
                      .then((result) => {
                        if (result.state === "current") {
                          setNote(`Up to date. ${result.version}.`);
                          return;
                        }
                        if (result.state === "blocked") {
                          const why =
                            result.reason === "mismatch"
                              ? "The installer hash does not match the release."
                              : "No trusted checksum was published with this release.";
                          setNote(`Update blocked. ${why}`);
                          return;
                        }
                        if (result.state !== "ready") {
                          setNote("Update did not start.");
                          return;
                        }
                        const install = window.confirm(`Install Daylight ${result.latest}? The installer hash matches the release.`);
                        if (!install) {
                          setNote("Update not installed.");
                          return;
                        }
                        setNote(`Installing ${result.latest}. Daylight will close and reopen.`);
                        return invoke("install_verified_update");
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
