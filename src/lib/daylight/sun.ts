/** Compact SunCalc (NOAA-style) sunrise, solar noon, and sunset. No network. */

const PI = Math.PI;
const RAD = PI / 180;
const DAY_MS = 86_400_000;
const J1970 = 2_440_588;
const J2000 = 2_451_545;

function toDays(date: Date): number {
  return date.valueOf() / DAY_MS - 0.5 + J1970 - J2000;
}

function fromJulian(j: number): Date {
  return new Date((j + 0.5 - J1970) * DAY_MS);
}

function declination(l: number): number {
  return Math.asin(Math.sin(l) * Math.sin(RAD * 23.4397));
}

function solarMeanAnomaly(d: number): number {
  return RAD * (357.5291 + 0.98560028 * d);
}

function eclipticLongitude(M: number): number {
  const C = RAD * (1.9148 * Math.sin(M) + 0.02 * Math.sin(2 * M) + 0.0003 * Math.sin(3 * M));
  return M + C + RAD * 102.9372 + PI;
}

function hourAngle(h: number, phi: number, dec: number): number {
  const cosH = (Math.sin(h) - Math.sin(phi) * Math.sin(dec)) / (Math.cos(phi) * Math.cos(dec));
  if (cosH < -1 || cosH > 1 || Number.isNaN(cosH)) return Number.NaN;
  return Math.acos(cosH);
}

function julianCycle(d: number, lw: number): number {
  return Math.round(d - 0.0009 - lw / (2 * PI));
}

function approxTransit(Ht: number, lw: number, n: number): number {
  return 0.0009 + (Ht + lw) / (2 * PI) + n;
}

function solarTransitJ(ds: number, M: number, L: number): number {
  return J2000 + ds + 0.0053 * Math.sin(M) - 0.0069 * Math.sin(2 * L);
}

export type SunTimes = {
  sunrise: Date;
  sunset: Date;
  solarNoon: Date;
};

/** Times for the solar day containing `instant`. Null inside polar day or night. */
export function sunTimes(instant: Date, lat: number, lon: number): SunTimes | null {
  const lw = RAD * -lon;
  const phi = RAD * lat;
  const d = toDays(instant);
  const n = julianCycle(d, lw);
  const ds = approxTransit(0, lw, n);
  const M = solarMeanAnomaly(ds);
  const L = eclipticLongitude(M);
  const dec = declination(L);
  const Jnoon = solarTransitJ(ds, M, L);
  const h0 = -0.833 * RAD;
  const w = hourAngle(h0, phi, dec);
  if (Number.isNaN(w)) return null;
  const a = approxTransit(w, lw, n);
  const Jset = solarTransitJ(a, M, L);
  const Jrise = Jnoon - (Jset - Jnoon);
  return {
    sunrise: fromJulian(Jrise),
    sunset: fromJulian(Jset),
    solarNoon: fromJulian(Jnoon),
  };
}

function zoneParts(timeZone: string, date: Date) {
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone,
    hourCycle: "h23",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).formatToParts(date);
  const map = Object.fromEntries(parts.map((part) => [part.type, part.value]));
  let hour = Number(map.hour);
  if (hour === 24) hour = 0;
  return {
    year: Number(map.year),
    month: Number(map.month),
    day: Number(map.day),
    hour,
    minute: Number(map.minute),
    second: Number(map.second),
  };
}

function offsetMs(timeZone: string, date: Date): number {
  const p = zoneParts(timeZone, date);
  const asUtc = Date.UTC(p.year, p.month - 1, p.day, p.hour, p.minute, p.second);
  return asUtc - date.getTime();
}

/** Civil noon in `timeZone` for the calendar day of `now` in that zone. */
export function zonedNoon(now: Date, timeZone: string): Date {
  const p = zoneParts(timeZone, now);
  const guess = new Date(Date.UTC(p.year, p.month - 1, p.day, 12, 0, 0));
  const first = offsetMs(timeZone, guess);
  let utc = new Date(guess.getTime() - first);
  const second = offsetMs(timeZone, utc);
  if (second !== first) utc = new Date(guess.getTime() - second);
  return utc;
}

export function sunTimesOnLocalDay(now: Date, lat: number, lon: number, timeZone: string): SunTimes | null {
  return sunTimes(zonedNoon(now, timeZone), lat, lon);
}

export function formatClock(date: Date, timeZone: string): string {
  const p = zoneParts(timeZone, date);
  return `${String(p.hour).padStart(2, "0")}:${String(p.minute).padStart(2, "0")}`;
}

export function dayFraction(now: Date, sunrise: Date, sunset: Date): number {
  const span = sunset.getTime() - sunrise.getTime();
  if (span <= 0) return 0;
  return Math.min(1, Math.max(0, (now.getTime() - sunrise.getTime()) / span));
}
