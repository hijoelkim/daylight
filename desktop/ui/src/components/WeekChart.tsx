import { useEffect, useState } from "react";
import { Bar, BarChart, ResponsiveContainer, Tooltip, XAxis } from "recharts";

export type WeekDay = { day: string; hours: number };

export function WeekChart({ days }: { days: WeekDay[] }) {
  const [ready, setReady] = useState(false);
  useEffect(() => setReady(true), []);

  return (
    <div>
      <p className="text-sm text-muted">Hours on screen.</p>
      <ul className="sr-only">
        {days.map((day) => (
          <li key={day.day}>
            {day.day} {day.hours.toFixed(1)} hours
          </li>
        ))}
      </ul>
      <div className="mt-4 h-56">
        {ready ? (
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={days} margin={{ top: 8, right: 0, left: 0, bottom: 0 }}>
              <XAxis
                dataKey="day"
                tick={{ fill: "var(--color-muted)", fontSize: 12 }}
                axisLine={{ stroke: "var(--color-border)" }}
                tickLine={false}
              />
              <Tooltip
                cursor={{ fill: "var(--color-bg-subtle)" }}
                contentStyle={{
                  background: "var(--color-bg-elevated)",
                  border: "1px solid var(--color-border)",
                  color: "var(--color-fg)",
                  fontFamily: "var(--font-mono)",
                  fontSize: 12,
                }}
                formatter={(value) => [`${Number(value).toFixed(1)}h`, "On screen"]}
              />
              <Bar dataKey="hours" fill="var(--color-accent)" maxBarSize={32} isAnimationActive={false} />
            </BarChart>
          </ResponsiveContainer>
        ) : null}
      </div>
    </div>
  );
}
