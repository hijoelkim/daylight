import { formatMinutes, type SampleApp } from "@/lib/daylight/demo-data";

export function AppList({
  dateLabel,
  apps,
  empty,
}: {
  dateLabel: string;
  apps: SampleApp[];
  empty: boolean;
}) {
  const max = Math.max(...apps.map((app) => app.minutes), 1);

  return (
    <section className="mt-10" aria-labelledby="apps-heading">
      <div className="flex items-baseline justify-between gap-4">
        <h2 id="apps-heading" className="font-display text-2xl tracking-wide">
          Apps
        </h2>
        <p className="font-mono text-xs text-muted">{dateLabel}</p>
      </div>
      {empty ? (
        <p className="mt-4 text-muted">No apps yet.</p>
      ) : (
        <ul className="mt-4 space-y-4">
          {apps.map((app) => (
            <li key={app.name} className="grid grid-cols-[0.75rem_1fr] gap-x-3">
              <span className="mt-1.5 size-2.5" style={{ backgroundColor: app.color }} aria-hidden="true" />
              <div className="flex items-baseline justify-between gap-4">
                <span>{app.name}</span>
                <span className="font-mono text-sm tabular-nums text-muted">{formatMinutes(app.minutes)}</span>
              </div>
              <div className="col-start-2 mt-2 h-1 bg-bg-subtle" aria-hidden="true">
                <div className="h-full" style={{ width: `${(app.minutes / max) * 100}%`, backgroundColor: app.color }} />
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
