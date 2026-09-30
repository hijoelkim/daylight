import { formatMs } from "../format";

export function ScreenBattery({ usedMs, budgetMin }: { usedMs: number; budgetMin: number }) {
  if (budgetMin <= 0) return null;
  const total = budgetMin * 60_000;
  const left = Math.max(0, Math.min(1, 1 - usedMs / total));
  const remaining = Math.max(0, total - usedMs);

  return (
    <div className="mt-4 flex items-center gap-4">
      <div className="flex items-center" aria-hidden="true">
        <div className="h-8 w-36 border border-accent bg-bg p-0.5">
          <div className="h-full bg-[#7dba78]" style={{ width: `${left * 100}%` }} />
        </div>
        <div className="h-3 w-1 bg-accent" />
      </div>
      <p className="font-mono text-sm tabular-nums text-muted">
        {remaining <= 0 ? "0h 00m left" : `${formatMs(remaining)} left`}
      </p>
    </div>
  );
}
