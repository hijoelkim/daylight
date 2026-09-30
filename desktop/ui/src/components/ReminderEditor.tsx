const kinds = ["interval", "clock", "after-screen", "after-app", "sunset", "once"] as const;

const labels: Record<string, string> = {
  interval: "Every N minutes",
  clock: "At a clock time",
  "after-screen": "After screen time",
  "after-app": "After an app",
  sunset: "At sunset",
  once: "Once",
};

const fieldClass =
  "min-h-11 w-full border border-border bg-bg px-3 text-fg outline-none focus-visible:border-accent";

export type ReminderRow = {
  id: number;
  kind: string;
  title: string;
  body: string;
  interval_min: number | null;
  time_local: string | null;
  after_screen_min: number | null;
  app_key: string | null;
  sunset_offset_min: number | null;
  enabled: number;
};

function summary(reminder: ReminderRow): string {
  if (reminder.kind === "interval") return `every ${reminder.interval_min ?? 60} minutes`;
  if (reminder.kind === "clock") return `at ${reminder.time_local ?? ""}`;
  if (reminder.kind === "after-screen") return `after ${reminder.after_screen_min ?? 30} minutes on screen`;
  if (reminder.kind === "after-app") return `after ${reminder.after_screen_min ?? 30} minutes in ${reminder.app_key || "an app"}`;
  if (reminder.kind === "sunset") return `sunset ${reminder.sunset_offset_min ?? 0} min`;
  return reminder.time_local || "once";
}

export function ReminderEditor({
  reminders,
  onSave,
  onDelete,
  onTest,
  onAdd,
}: {
  reminders: ReminderRow[];
  onSave: (reminder: ReminderRow) => void;
  onDelete: (id: number) => void;
  onTest: (id: number) => void;
  onAdd: () => void;
}) {
  return (
    <div className="space-y-4">
      {reminders.map((reminder) => (
        <article key={reminder.id} className="border border-border bg-bg-elevated p-4">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0">
              <p className="truncate font-display text-2xl tracking-wide">{reminder.title || "Untitled"}</p>
              <p className="font-mono text-xs text-muted">{summary(reminder)}</p>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={reminder.enabled === 1}
              aria-label={reminder.enabled === 1 ? "Enabled" : "Off"}
              onClick={() => onSave({ ...reminder, enabled: reminder.enabled === 1 ? 0 : 1 })}
              className={`relative mt-1 h-6 w-10 shrink-0 border ${reminder.enabled === 1 ? "border-accent bg-bg-subtle" : "border-border bg-bg"}`}
            >
              <span className={`absolute top-0.5 left-0.5 size-4 bg-accent ${reminder.enabled === 1 ? "translate-x-4" : ""}`} />
            </button>
          </div>
          <div className="mt-4 grid gap-3">
            <label className="grid gap-1 text-sm">
              <span className="text-muted">Title</span>
              <input className={fieldClass} value={reminder.title} onChange={(event) => onSave({ ...reminder, title: event.target.value })} />
            </label>
            <label className="grid gap-1 text-sm">
              <span className="text-muted">Body</span>
              <input className={fieldClass} value={reminder.body} onChange={(event) => onSave({ ...reminder, body: event.target.value })} />
            </label>
            <label className="grid gap-1 text-sm">
              <span className="text-muted">Kind</span>
              <select className={fieldClass} value={reminder.kind} onChange={(event) => onSave({ ...reminder, kind: event.target.value })}>
                {kinds.map((kind) => (
                  <option key={kind} value={kind}>
                    {labels[kind]}
                  </option>
                ))}
              </select>
            </label>
            {reminder.kind === "interval" ? (
              <NumberField label="Interval minutes" value={reminder.interval_min ?? 60} onChange={(interval_min) => onSave({ ...reminder, interval_min })} />
            ) : null}
            {reminder.kind === "clock" ? (
              <label className="grid gap-1 text-sm">
                <span className="text-muted">Clock time</span>
                <input type="time" className={fieldClass} value={reminder.time_local ?? "09:00"} onChange={(event) => onSave({ ...reminder, time_local: event.target.value })} />
              </label>
            ) : null}
            {reminder.kind === "once" ? (
              <label className="grid gap-1 text-sm">
                <span className="text-muted">Date and time</span>
                <input className={fieldClass} placeholder="2026-09-30 18:00" value={reminder.time_local ?? ""} onChange={(event) => onSave({ ...reminder, time_local: event.target.value })} />
              </label>
            ) : null}
            {reminder.kind === "after-screen" || reminder.kind === "after-app" ? (
              <NumberField label="After minutes" value={reminder.after_screen_min ?? 30} onChange={(after_screen_min) => onSave({ ...reminder, after_screen_min })} />
            ) : null}
            {reminder.kind === "after-app" ? (
              <label className="grid gap-1 text-sm">
                <span className="text-muted">App</span>
                <input className={fieldClass} placeholder="chrome.exe" value={reminder.app_key ?? ""} onChange={(event) => onSave({ ...reminder, app_key: event.target.value })} />
              </label>
            ) : null}
            {reminder.kind === "sunset" ? (
              <NumberField label="Minutes after sunset" value={reminder.sunset_offset_min ?? 0} onChange={(sunset_offset_min) => onSave({ ...reminder, sunset_offset_min })} />
            ) : null}
          </div>
          <div className="mt-4 flex gap-4">
            <button type="button" className="inline-flex min-h-11 items-center text-sm text-fg" onClick={() => onTest(reminder.id)}>
              Test ping
            </button>
            <button type="button" className="inline-flex min-h-11 items-center text-sm text-muted" onClick={() => onDelete(reminder.id)}>
              Remove
            </button>
          </div>
        </article>
      ))}
      <button type="button" className="inline-flex min-h-11 items-center text-sm text-accent" onClick={onAdd}>
        Add reminder
      </button>
    </div>
  );
}

function NumberField({ label, value, onChange }: { label: string; value: number; onChange: (value: number) => void }) {
  return (
    <label className="grid gap-1 text-sm">
      <span className="text-muted">{label}</span>
      <input type="number" className={fieldClass} value={value} onChange={(event) => onChange(Number(event.target.value) || 0)} />
    </label>
  );
}
