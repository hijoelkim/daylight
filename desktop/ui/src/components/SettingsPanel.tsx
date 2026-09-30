import type { ReactNode } from "react";

const fieldClass =
  "min-h-11 border border-border bg-bg px-3 text-fg outline-none focus-visible:border-accent";

export type LiveSettings = {
  idle_threshold_s: string;
  retain_days: string;
  record_titles: string;
  start_with_windows: string;
  city: string;
  lat: string;
  lon: string;
  screen_budget_min: string;
  hardcore: string;
  auto_dim: string;
};

export function SettingsPanel({
  settings,
  paused,
  onChange,
  onExport,
  onWipe,
  onPause,
  onUpdate,
}: {
  settings: LiveSettings;
  paused: boolean;
  onChange: (patch: Partial<LiveSettings>) => void;
  onExport: () => void;
  onWipe: () => void;
  onPause: () => void;
  onUpdate: () => void;
}) {
  return (
    <div>
      <SettingRow label="Maximum screen time">
        <BudgetFields minutes={Number(settings.screen_budget_min) || 0} onChange={(minutes) => onChange({ screen_budget_min: String(minutes) })} />
      </SettingRow>
      <SettingRow label="Hardcore mode">
        <span className="flex flex-col items-end gap-2">
          <input
            type="checkbox"
            checked={settings.hardcore === "1"}
            onChange={(event) => onChange({ hardcore: event.target.checked ? "1" : "0" })}
            className="size-4 accent-fg"
          />
          <details className="max-w-sm text-right">
            <summary className="cursor-pointer text-sm text-muted">What hardcore mode does</summary>
            <p className="mt-2 text-pretty text-sm leading-relaxed text-muted">
              Hardcore mode warns at 10% left, then again at 2%. When the battery reaches zero, the computer goes to sleep.
            </p>
          </details>
        </span>
      </SettingRow>
      <SettingRow label="Auto dim with rise and set">
        <span className="flex flex-col items-end gap-2">
          <input
            type="checkbox"
            checked={settings.auto_dim === "1"}
            onChange={(event) => onChange({ auto_dim: event.target.checked ? "1" : "0" })}
            className="size-4 accent-fg"
          />
          <details className="max-w-sm text-right">
            <summary className="cursor-pointer text-sm text-muted">What auto dim does</summary>
            <p className="mt-2 text-pretty text-sm leading-relaxed text-muted">
              From sunset, the screen dims by 30 over 30 minutes. From sunrise, it returns to your brightness over 30 minutes.
            </p>
          </details>
        </span>
      </SettingRow>
      <SettingRow label="Idle threshold">
        <span className="flex items-center gap-2">
          <input type="number" min={30} max={300} className={`${fieldClass} w-24 text-right font-mono`} value={settings.idle_threshold_s} onChange={(event) => onChange({ idle_threshold_s: event.target.value })} />
          <span className="font-mono text-sm text-muted">s</span>
        </span>
      </SettingRow>
      <SettingRow label="Keep history">
        <span className="flex items-center gap-2">
          <input type="number" min={1} max={3650} className={`${fieldClass} w-24 text-right font-mono`} value={settings.retain_days} onChange={(event) => onChange({ retain_days: event.target.value })} />
          <span className="font-mono text-sm text-muted">days</span>
        </span>
      </SettingRow>
      <SettingRow label="Record window titles">
        <button type="button" role="switch" aria-checked={settings.record_titles === "1"} onClick={() => onChange({ record_titles: settings.record_titles === "1" ? "0" : "1" })} className={`relative h-6 w-10 border ${settings.record_titles === "1" ? "border-accent bg-bg-subtle" : "border-border bg-bg"}`}>
          <span className={`absolute top-0.5 left-0.5 size-4 bg-accent ${settings.record_titles === "1" ? "translate-x-4" : ""}`} />
        </button>
      </SettingRow>
      <SettingRow label="Start with Windows">
        <button type="button" role="switch" aria-checked={settings.start_with_windows === "1"} onClick={() => onChange({ start_with_windows: settings.start_with_windows === "1" ? "0" : "1" })} className={`relative h-6 w-10 border ${settings.start_with_windows === "1" ? "border-accent bg-bg-subtle" : "border-border bg-bg"}`}>
          <span className={`absolute top-0.5 left-0.5 size-4 bg-accent ${settings.start_with_windows === "1" ? "translate-x-4" : ""}`} />
        </button>
      </SettingRow>
      <SettingRow label="City">
        <input className={`${fieldClass} w-40`} value={settings.city} placeholder="Sydney or lat,lon" onChange={(event) => onChange({ city: event.target.value })} />
      </SettingRow>
      <SettingRow label="Latitude">
        <input className={`${fieldClass} w-32 text-right font-mono`} value={settings.lat} onChange={(event) => onChange({ lat: event.target.value })} />
      </SettingRow>
      <SettingRow label="Longitude">
        <input className={`${fieldClass} w-32 text-right font-mono`} value={settings.lon} onChange={(event) => onChange({ lon: event.target.value })} />
      </SettingRow>
      <div className="mt-6 flex flex-wrap gap-4">
        <button type="button" className="inline-flex min-h-11 items-center text-sm text-fg" onClick={onPause}>
          {paused ? "Resume recording" : "Pause recording"}
        </button>
        <button type="button" className="inline-flex min-h-11 items-center text-sm text-fg" onClick={onUpdate}>
          Check for updates
        </button>
        <button type="button" className="inline-flex min-h-11 items-center text-sm text-fg" onClick={onExport}>
          Export
        </button>
        <button type="button" className="inline-flex min-h-11 items-center text-sm text-danger" onClick={onWipe}>
          Wipe history
        </button>
      </div>
    </div>
  );
}

function BudgetFields({ minutes, onChange }: { minutes: number; onChange: (minutes: number) => void }) {
  const hours = Math.floor(Math.max(0, minutes) / 60);
  const rest = Math.max(0, minutes) % 60;
  return (
    <span className="flex items-center gap-2">
      <input
        type="number"
        min={0}
        max={24}
        className={`${fieldClass} w-20 text-right font-mono`}
        value={hours}
        onChange={(event) => onChange(Math.min(24, Math.max(0, Number(event.target.value) || 0)) * 60 + rest)}
      />
      <span className="font-mono text-sm text-muted">h</span>
      <input
        type="number"
        min={0}
        max={59}
        className={`${fieldClass} w-20 text-right font-mono`}
        value={rest}
        onChange={(event) => onChange(hours * 60 + Math.min(59, Math.max(0, Number(event.target.value) || 0)))}
      />
      <span className="font-mono text-sm text-muted">m</span>
    </span>
  );
}

function SettingRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 border-t border-border py-4">
      <span>{label}</span>
      {children}
    </div>
  );
}
