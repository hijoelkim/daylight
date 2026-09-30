import { z } from "zod";

export const reminderKinds = [
  "interval",
  "clock",
  "after-screen",
  "after-app",
  "sunset",
  "once",
] as const;

export type ReminderKind = (typeof reminderKinds)[number];

export type Reminder = {
  id: string;
  title: string;
  body: string;
  kind: ReminderKind;
  intervalMin: number;
  timeLocal: string;
  afterScreenMin: number;
  enabled: boolean;
};

export type SampleApp = {
  name: string;
  minutes: number;
  chip: string;
};

export const sampleDayLabel = "Tue 29 Sep";

export const sampleScreen = "3h 12m on screen";
export const sampleDaylightPct = 28;
export const sampleSecondary = "1h 04m idle · 42m locked";
export const emptyLine = "Leave it in the tray. Come back this evening.";

export const sampleApps: SampleApp[] = [
  { name: "Cursor", minutes: 88, chip: "bg-accent" },
  { name: "Google Chrome", minutes: 54, chip: "bg-muted" },
  { name: "Windows Explorer", minutes: 22, chip: "bg-warn" },
  { name: "Spotify", minutes: 18, chip: "bg-accent/70" },
  { name: "Windows Terminal", minutes: 10, chip: "bg-warn/60" },
];

export const sampleWeek = [
  { day: "Mon", hours: 4.1 },
  { day: "Tue", hours: 3.2 },
  { day: "Wed", hours: 5.0 },
  { day: "Thu", hours: 2.8 },
  { day: "Fri", hours: 6.2 },
  { day: "Sat", hours: 1.4 },
  { day: "Sun", hours: 0.9 },
] as const;

export const cities = [
  { id: "sydney", name: "Sydney", lat: -33.8688, lon: 151.2093, timeZone: "Australia/Sydney" },
  { id: "london", name: "London", lat: 51.5074, lon: -0.1278, timeZone: "Europe/London" },
  { id: "new-york", name: "New York", lat: 40.7128, lon: -74.006, timeZone: "America/New_York" },
  { id: "tokyo", name: "Tokyo", lat: 35.6762, lon: 139.6503, timeZone: "Asia/Tokyo" },
] as const;

export type CityId = (typeof cities)[number]["id"] | "custom";

export const defaultReminders: Reminder[] = [
  {
    id: "stand-up",
    title: "Stand up",
    body: "Get out of the chair.",
    kind: "interval",
    intervalMin: 60,
    timeLocal: "09:00",
    afterScreenMin: 30,
    enabled: true,
  },
  {
    id: "light-is-going",
    title: "Light is going",
    body: "The sun is down soon.",
    kind: "sunset",
    intervalMin: 60,
    timeLocal: "18:00",
    afterScreenMin: 30,
    enabled: true,
  },
];

export const kindLabels: Record<ReminderKind, string> = {
  interval: "Every N minutes",
  clock: "At a clock time",
  "after-screen": "After screen time",
  "after-app": "After an app",
  sunset: "At sunset",
  once: "Once",
};

const latLon = z.object({
  lat: z.number().gte(-90).lte(90),
  lon: z.number().gte(-180).lte(180),
});

export function parseLatLon(lat: string, lon: string): { lat: number; lon: number } | null {
  if (lat.trim() === "" || lon.trim() === "") return null;
  const parsed = latLon.safeParse({ lat: Number(lat), lon: Number(lon) });
  return parsed.success ? parsed.data : null;
}

export function formatMinutes(minutes: number): string {
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return `${h}h ${String(m).padStart(2, "0")}m`;
}

export function reminderSummary(reminder: Reminder): string {
  switch (reminder.kind) {
    case "interval":
      return `every ${reminder.intervalMin} minutes`;
    case "clock":
      return `at ${reminder.timeLocal}`;
    case "after-screen":
      return `after ${reminder.afterScreenMin} minutes on screen`;
    case "after-app":
      return `after ${reminder.afterScreenMin} minutes on an app`;
    case "sunset":
      return "at sunset";
    case "once":
      return `once at ${reminder.timeLocal}`;
  }
}

export const consentText =
  "The installed app records which app is in front. Data stays in %LOCALAPPDATA%\\Daylight. No cloud. No keystrokes. No screenshots.";
