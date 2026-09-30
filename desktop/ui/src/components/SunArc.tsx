function quad(t: number, a: number, b: number, c: number): number {
  const u = 1 - t;
  return u * u * a + 2 * u * t * b + t * t * c;
}

const X0 = 28;
const Y0 = 112;
const X1 = 160;
const Y1 = 20;
const X2 = 292;
const Y2 = 112;

function point(t: number) {
  return { x: quad(t, X0, X1, X2), y: quad(t, Y0, Y1, Y2) };
}

export type ArcMark = { t0: number; t1: number; color: string };

function arcPath(t0: number, t1: number) {
  const span = Math.max(0, t1 - t0);
  const steps = Math.max(2, Math.ceil(span * 64));
  let path = "";
  for (let index = 0; index <= steps; index += 1) {
    const t = t0 + (span * index) / steps;
    const p = point(t);
    path += `${index === 0 ? "M" : "L"}${p.x.toFixed(2)} ${p.y.toFixed(2)}`;
  }
  return path;
}

export function SunArc({
  rise,
  set,
  noonT,
  nowT,
  marks = [],
}: {
  rise: string;
  set: string;
  noonT: number;
  nowT: number;
  marks?: ArcMark[];
}) {
  const start = point(0);
  const end = point(1);
  const noon = point(Math.min(1, Math.max(0, noonT)));
  const now = point(Math.min(1, Math.max(0, nowT)));

  return (
    <figure>
      <div className="relative bg-horizon-sky">
        <div className="absolute inset-x-0 bottom-0 h-8 bg-horizon-ground" />
        <svg viewBox="0 0 320 148" className="relative block h-auto w-full" role="img" aria-label={`${rise} rise, ${set} set`}>
          <path d={arcPath(0, 1)} fill="none" stroke="#3a4454" strokeWidth="3" />
          {marks.map((mark) => (
            <path
              key={`${mark.color}-${mark.t0}-${mark.t1}`}
              d={arcPath(mark.t0, mark.t1)}
              fill="none"
              stroke={mark.color}
              strokeWidth="5"
              strokeLinecap="round"
            />
          ))}
          <circle cx={start.x} cy={start.y} r="3" className="fill-accent" />
          <circle cx={noon.x} cy={noon.y} r="2.5" className="fill-muted" />
          <circle cx={end.x} cy={end.y} r="3" className="fill-muted" />
          <circle cx={now.x} cy={now.y} r="5" className="fill-fg" />
        </svg>
      </div>
      <figcaption className="mt-3 flex flex-wrap gap-x-8 gap-y-1 font-mono text-sm tabular-nums">
        <span>{rise} rise</span>
        <span>{set} set</span>
      </figcaption>
    </figure>
  );
}
