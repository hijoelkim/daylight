import type { ReactNode } from "react";
import { consentText } from "@/lib/daylight/demo-data";

const fieldClass =
  "min-h-11 w-24 border border-border bg-bg px-3 text-right font-mono text-fg outline-none focus-visible:border-accent";

export function SettingsPanel({
  idleSec,
  retainDays,
  recordTitles,
  onIdle,
  onRetain,
  onTitles,
}: {
  idleSec: number;
  retainDays: number;
  recordTitles: boolean;
  onIdle: (seconds: number) => void;
  onRetain: (days: number) => void;
  onTitles: (on: boolean) => void;
}) {
  return (
    <div>
      <SettingRow label="Idle threshold">
        <span className="flex items-center gap-2">
          <input
            type="number"
            min={30}
            max={300}
            className={fieldClass}
            value={idleSec}
            onChange={(event) => onIdle(Number(event.target.value))}
          />
          <span className="font-mono text-sm text-muted">s</span>
        </span>
      </SettingRow>
      <SettingRow label="Keep history">
        <span className="flex items-center gap-2">
          <input
            type="number"
            min={1}
            max={3650}
            className={fieldClass}
            value={retainDays}
            onChange={(event) => onRetain(Number(event.target.value))}
          />
          <span className="font-mono text-sm text-muted">days</span>
        </span>
      </SettingRow>
      <SettingRow label="Record window titles">
        <button
          type="button"
          role="switch"
          aria-checked={recordTitles}
          onClick={() => onTitles(!recordTitles)}
          className={`relative h-6 w-10 border ${recordTitles ? "border-accent bg-bg-subtle" : "border-border bg-bg"}`}
        >
          <span className={`absolute top-0.5 left-0.5 size-4 bg-accent ${recordTitles ? "translate-x-4" : ""}`} />
        </button>
      </SettingRow>
      <SettingRow label="Start with Windows">
        <span className="font-mono text-sm text-muted">Off · installed app only</span>
      </SettingRow>
      <p className="mt-6 border-l border-border pl-4 text-pretty text-sm leading-relaxed text-muted">{consentText}</p>
    </div>
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
