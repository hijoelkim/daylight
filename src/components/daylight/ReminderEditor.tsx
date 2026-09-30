import {
  kindLabels,
  reminderKinds,
  reminderSummary,
  type Reminder,
  type ReminderKind,
} from "@/lib/daylight/demo-data";

const fieldClass =
  "min-h-11 w-full border border-border bg-bg px-3 text-fg outline-none focus-visible:border-accent";

export function ReminderEditor({
  reminders,
  onChange,
  onTest,
}: {
  reminders: Reminder[];
  onChange: (reminders: Reminder[]) => void;
  onTest: (reminder: Reminder) => void;
}) {
  function update(id: string, patch: Partial<Reminder>) {
    onChange(reminders.map((reminder) => (reminder.id === id ? { ...reminder, ...patch } : reminder)));
  }

  function add() {
    onChange([
      ...reminders,
      {
        id: crypto.randomUUID(),
        title: "",
        body: "",
        kind: "interval",
        intervalMin: 60,
        timeLocal: "09:00",
        afterScreenMin: 30,
        enabled: true,
      },
    ]);
  }

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted">Saved in this browser only. This page cannot fire a Windows toast.</p>
      {reminders.map((reminder) => (
        <article key={reminder.id} className="border border-border bg-bg-elevated p-4">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0">
              <p className="truncate font-display text-2xl tracking-wide">{reminder.title || "Untitled"}</p>
              <p className="font-mono text-xs text-muted">{reminderSummary(reminder)}</p>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={reminder.enabled}
              aria-label={reminder.enabled ? "Enabled" : "Off"}
              onClick={() => update(reminder.id, { enabled: !reminder.enabled })}
              className={`relative mt-1 h-6 w-10 shrink-0 border ${reminder.enabled ? "border-accent bg-bg-subtle" : "border-border bg-bg"}`}
            >
              <span
                className={`absolute top-0.5 left-0.5 size-4 bg-accent ${reminder.enabled ? "translate-x-4" : ""}`}
              />
            </button>
          </div>
          <div className="mt-4 grid gap-3">
            <label className="grid gap-1 text-sm">
              <span className="text-muted">Title</span>
              <input
                className={fieldClass}
                value={reminder.title}
                onChange={(event) => update(reminder.id, { title: event.target.value })}
              />
            </label>
            <label className="grid gap-1 text-sm">
              <span className="text-muted">Body</span>
              <input
                className={fieldClass}
                value={reminder.body}
                onChange={(event) => update(reminder.id, { body: event.target.value })}
              />
            </label>
            <label className="grid gap-1 text-sm">
              <span className="text-muted">Kind</span>
              <select
                className={fieldClass}
                value={reminder.kind}
                onChange={(event) => update(reminder.id, { kind: event.target.value as ReminderKind })}
              >
                {reminderKinds.map((kind) => (
                  <option key={kind} value={kind}>
                    {kindLabels[kind]}
                  </option>
                ))}
              </select>
            </label>
            {reminder.kind === "interval" ? (
              <NumberField
                label="Interval minutes"
                value={reminder.intervalMin}
                onChange={(intervalMin) => update(reminder.id, { intervalMin })}
              />
            ) : null}
            {reminder.kind === "clock" || reminder.kind === "once" ? (
              <label className="grid gap-1 text-sm">
                <span className="text-muted">Clock time</span>
                <input
                  type="time"
                  className={fieldClass}
                  value={reminder.timeLocal}
                  onChange={(event) => update(reminder.id, { timeLocal: event.target.value })}
                />
              </label>
            ) : null}
            {reminder.kind === "after-screen" || reminder.kind === "after-app" ? (
              <NumberField
                label="After minutes"
                value={reminder.afterScreenMin}
                onChange={(afterScreenMin) => update(reminder.id, { afterScreenMin })}
              />
            ) : null}
          </div>
          <div className="mt-4 flex gap-4">
            <button
              type="button"
              className="inline-flex min-h-11 items-center text-sm text-fg"
              onClick={() => onTest(reminder)}
            >
              Test ping
            </button>
            <button
              type="button"
              className="inline-flex min-h-11 items-center text-sm text-muted"
              onClick={() => onChange(reminders.filter((item) => item.id !== reminder.id))}
            >
              Remove
            </button>
          </div>
        </article>
      ))}
      <button type="button" className="inline-flex min-h-11 items-center text-sm text-accent" onClick={add}>
        Add reminder
      </button>
    </div>
  );
}

function NumberField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="grid gap-1 text-sm">
      <span className="text-muted">{label}</span>
      <input
        type="number"
        min={1}
        className={fieldClass}
        value={value}
        onChange={(event) => onChange(Math.max(1, Number(event.target.value) || 1))}
      />
    </label>
  );
}
