import { formatMs } from "../format";

function mix(a: number[], b: number[], t: number) {
  return a.map((channel, index) => Math.round(channel + (b[index] - channel) * t));
}

function charge(left: number) {
  const green = [61, 158, 78];
  const amber = [214, 176, 64];
  const red = [196, 74, 64];
  const rgb = left >= 0.5 ? mix(amber, green, (left - 0.5) * 2) : mix(red, amber, left * 2);
  const light = rgb.map((channel) => Math.min(255, channel + 48));
  return { solid: `rgb(${rgb.join(",")})`, light: `rgb(${light.join(",")})` };
}

export function ScreenBattery({ usedMs, budgetMin }: { usedMs: number; budgetMin: number }) {
  if (budgetMin <= 0) return null;
  const total = budgetMin * 60_000;
  const left = Math.max(0, Math.min(1, 1 - usedMs / total));
  const percent = Math.round(left * 100);
  const remaining = Math.max(0, total - usedMs);
  const color = charge(left);

  return (
    <div className="mt-4 flex items-center gap-4">
      <div className="flex items-center" role="img" aria-label={`${percent}% of today's screen time left`}>
        <div className="relative h-8 w-40 border border-accent bg-bg p-0.5">
          <div
            className="h-full"
            style={{
              width: `${percent}%`,
              background: `linear-gradient(90deg, ${color.light}, ${color.solid})`,
            }}
          />
          <span className={`absolute inset-0 flex items-center justify-center font-mono text-xs tabular-nums ${percent > 42 ? "text-[#14160f]" : "text-fg"}`}>
            {percent}%
          </span>
        </div>
        <div className="h-3 w-1 bg-accent" />
      </div>
      <p className="font-mono text-sm tabular-nums text-muted">
        {remaining <= 0 ? "0h 00m left" : `${formatMs(remaining)} left`}
      </p>
    </div>
  );
}
