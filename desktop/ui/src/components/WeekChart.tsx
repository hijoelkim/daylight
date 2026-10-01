import { useEffect, useState } from "react";
import { Bar, BarChart, LabelList, ResponsiveContainer, Tooltip, XAxis } from "recharts";

export type WeekDay = { day: string; hours: number };

type WeekBar = WeekDay & { within: number; over: number; overMin: number };

function barsFor(days: WeekDay[], budgetMin: number): WeekBar[] {
  const budgetHours = budgetMin > 0 ? budgetMin / 60 : 0;
  return days.map((day) => {
    const over = budgetHours > 0 ? Math.max(0, day.hours - budgetHours) : 0;
    const overMin = Math.round(over * 60);
    return { ...day, within: day.hours - over, over, overMin };
  });
}

function OverLabel({
  x = 0,
  y = 0,
  width = 0,
  value,
}: {
  x?: number | string;
  y?: number | string;
  width?: number | string;
  value?: number | string;
}) {
  const minutes = Number(value);
  if (!minutes) return null;
  return (
    <text
      x={Number(x) + Number(width) / 2}
      y={Number(y) - 6}
      textAnchor="middle"
      fill="var(--color-danger)"
      fontSize={11}
      fontFamily="var(--font-mono)"
    >
      +{minutes}m
    </text>
  );
}

function Tip({ active, payload }: { active?: boolean; payload?: Array<{ payload?: WeekBar }> }) {
  const row = payload?.[0]?.payload;
  if (!active || !row) return null;
  return (
    <div
      style={{
        background: "var(--color-bg-elevated)",
        border: "1px solid var(--color-border)",
        color: "var(--color-fg)",
        fontFamily: "var(--font-mono)",
        fontSize: 12,
        padding: "8px 10px",
      }}
    >
      <p>{row.day}</p>
      <p>{row.hours.toFixed(1)}h on screen</p>
      {row.overMin > 0 ? <p style={{ color: "var(--color-danger)" }}>{row.overMin} min over the limit</p> : null}
    </div>
  );
}

export function WeekChart({ days, budgetMin }: { days: WeekDay[]; budgetMin: number }) {
  const [ready, setReady] = useState(false);
  useEffect(() => setReady(true), []);
  const bars = barsFor(days, budgetMin);

  return (
    <div>
      <p className="text-sm text-muted">
        Hours on screen.{budgetMin > 0 ? " Red is past the daily limit." : ""}
      </p>
      <ul className="sr-only">
        {bars.map((day) => (
          <li key={day.day}>
            {day.day} {day.hours.toFixed(1)} hours
            {day.overMin > 0 ? `, ${day.overMin} minutes over the limit` : ""}
          </li>
        ))}
      </ul>
      <div className="mt-4 h-56">
        {ready ? (
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={bars} margin={{ top: 22, right: 0, left: 0, bottom: 0 }}>
              <XAxis
                dataKey="day"
                tick={{ fill: "var(--color-muted)", fontSize: 12 }}
                axisLine={{ stroke: "var(--color-border)" }}
                tickLine={false}
              />
              <Tooltip cursor={{ fill: "var(--color-bg-subtle)" }} content={<Tip />} />
              <Bar dataKey="within" stackId="use" fill="var(--color-accent)" maxBarSize={32} isAnimationActive={false} />
              <Bar dataKey="over" stackId="use" fill="var(--color-danger)" maxBarSize={32} isAnimationActive={false}>
                <LabelList dataKey="overMin" content={<OverLabel />} />
              </Bar>
            </BarChart>
          </ResponsiveContainer>
        ) : null}
      </div>
    </div>
  );
}
