import { emptyLine } from "../format";

export function DayMeter({
  screen,
  daylightPct,
  secondary,
  empty,
  polar,
}: {
  screen: string;
  daylightPct: number | null;
  secondary: string;
  empty: boolean;
  polar: boolean;
}) {
  return (
    <div className="mt-6">
      <p className="font-mono text-2xl tabular-nums text-fg">{empty ? "0h 00m on screen" : screen}</p>
      {polar ? null : (
        <p className="mt-1 font-mono text-sm tabular-nums text-muted">{empty ? "0" : daylightPct}% of daylight</p>
      )}
      {empty ? (
        <p className="mt-3 max-w-md text-pretty leading-relaxed text-muted">{emptyLine}</p>
      ) : (
        <p className="mt-3 font-mono text-sm text-muted">{secondary}</p>
      )}
    </div>
  );
}
