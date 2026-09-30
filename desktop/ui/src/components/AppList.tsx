import { formatMs } from "../format";

export type LiveApp = {
  app_key: string;
  product_name: string;
  active_ms: number;
  color?: string;
};

const chips = ["#e8dcc8", "#c4924a", "#7ea38a", "#8aa4c5", "#c58a7a"];

export function AppList({
  dateLabel,
  apps,
  current,
}: {
  dateLabel: string;
  apps: LiveApp[];
  current: { app_key: string; product_name: string } | null;
}) {
  const max = Math.max(...apps.map((app) => app.active_ms), 1);

  return (
    <section className="mt-10" aria-labelledby="apps-heading">
      <div className="flex items-baseline justify-between gap-4">
        <h2 id="apps-heading" className="font-display text-2xl tracking-wide">
          Apps
        </h2>
        <p className="font-mono text-xs text-muted">{dateLabel}</p>
      </div>
      {current ? (
        <p className="mt-3 font-mono text-sm text-fg">
          Now · {current.product_name || current.app_key}
        </p>
      ) : null}
      {apps.length === 0 ? (
        <p className="mt-4 text-muted">No apps yet.</p>
      ) : (
        <ul className="mt-4 space-y-4">
          {apps.map((app, index) => {
            const color = app.color || chips[index % chips.length];
            return (
            <li key={app.app_key} className="grid grid-cols-[0.75rem_1fr] gap-x-3">
              <span className="mt-1.5 size-2.5" style={{ backgroundColor: color }} aria-hidden="true" />
              <div className="flex items-baseline justify-between gap-4">
                <span>{app.product_name || app.app_key}</span>
                <span className="font-mono text-sm tabular-nums text-muted">{formatMs(app.active_ms)}</span>
              </div>
              <div className="col-start-2 mt-2 h-1 bg-bg-subtle" aria-hidden="true">
                <div className="h-full" style={{ width: `${(app.active_ms / max) * 100}%`, backgroundColor: color }} />
              </div>
            </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
