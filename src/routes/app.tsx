import { createFileRoute } from "@tanstack/react-router";
import { useEffect, useMemo, useState } from "react";
import { AppList } from "@/components/daylight/AppList";
import { DayMeter } from "@/components/daylight/DayMeter";
import { ReminderEditor } from "@/components/daylight/ReminderEditor";
import { SettingsPanel } from "@/components/daylight/SettingsPanel";
import { SiteChrome } from "@/components/daylight/SiteChrome";
import { SunArc } from "@/components/daylight/SunArc";
import { WeekChart } from "@/components/daylight/WeekChart";
import {
  cities,
  parseLatLon,
  sampleApps,
  sampleMarks,
  sampleDayLabel,
  sampleDaylightPct,
  sampleScreen,
  sampleSecondary,
  type CityId,
} from "@/lib/daylight/demo-data";
import { useDemo } from "@/lib/daylight/store";
import { dayFraction, formatClock, sunTimesOnLocalDay } from "@/lib/daylight/sun";

export const Route = createFileRoute("/app")({
  head: () => ({ meta: [{ title: "Today" }] }),
  component: Today,
});

const tabs = ["Week", "Reminders", "Settings"] as const;
type Tab = (typeof tabs)[number];

const fieldClass =
  "min-h-11 border border-border bg-bg-elevated px-3 text-fg outline-none focus-visible:border-accent";

function Today() {
  const demo = useDemo();
  const [tab, setTab] = useState<Tab>("Week");
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    demo.hydrate();
  }, [demo.hydrate]);

  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 30_000);
    return () => window.clearInterval(id);
  }, []);

  useEffect(() => {
    if (!demo.toast) return;
    const id = window.setTimeout(() => demo.dismissToast(), 4000);
    return () => window.clearTimeout(id);
  }, [demo.toast, demo.dismissToast]);

  const place = useMemo(() => resolvePlace(demo.cityId, demo.customLat, demo.customLon), [demo.cityId, demo.customLat, demo.customLon]);
  const times = useMemo(
    () => (place ? sunTimesOnLocalDay(now, place.lat, place.lon, place.timeZone) : null),
    [now, place],
  );

  const noonT = times ? dayFraction(times.solarNoon, times.sunrise, times.sunset) : 0.5;
  const nowT = times ? dayFraction(now, times.sunrise, times.sunset) : 0;
  const rise = times ? formatClock(times.sunrise, place?.timeZone ?? "UTC") : "--:--";
  const set = times ? formatClock(times.sunset, place?.timeZone ?? "UTC") : "--:--";

  return (
    <SiteChrome>
      <main className="mx-auto w-full max-w-4xl pb-8">
        <div className="mt-6 flex flex-wrap items-end justify-between gap-4">
          <div>
            <h1 className="font-display text-4xl font-medium tracking-wide sm:text-5xl">Today</h1>
            <p className="mt-2 text-sm text-muted">Sample day. This page does not watch your computer.</p>
          </div>
          <CityField
            cityId={demo.cityId}
            customLat={demo.customLat}
            customLon={demo.customLon}
            invalid={demo.cityId === "custom" && !parseLatLon(demo.customLat, demo.customLon)}
            onCity={demo.setCity}
            onCustom={demo.setCustom}
          />
        </div>

        <section className="mt-8" aria-label="Day arc">
          {place && times ? (
            <SunArc rise={rise} set={set} noonT={noonT} nowT={nowT} marks={demo.cleared ? [] : sampleMarks} />
          ) : (
            <p className="text-muted">
              {place ? "No sunrise or sunset for this place today." : "Use decimal degrees."}
            </p>
          )}
          <DayMeter
            screen={sampleScreen}
            daylightPct={sampleDaylightPct}
            secondary={sampleSecondary}
            empty={demo.cleared}
          />
          <button
            type="button"
            className="mt-4 inline-flex min-h-11 items-center text-sm text-muted"
            onClick={() => demo.setCleared(!demo.cleared)}
          >
            {demo.cleared ? "Show sample" : "Reset demo"}
          </button>
        </section>

        <AppList dateLabel={sampleDayLabel} apps={sampleApps} empty={demo.cleared} />

        <section className="mt-12" aria-label="More">
          <div role="tablist" aria-label="Today details" className="flex gap-2 border-b border-border">
            {tabs.map((name) => (
              <button
                key={name}
                type="button"
                role="tab"
                aria-selected={tab === name}
                onClick={() => setTab(name)}
                className={`inline-flex min-h-11 items-center border-b-2 px-3 font-display text-xl tracking-wide ${
                  tab === name ? "border-accent text-fg" : "border-transparent text-muted"
                }`}
              >
                {name}
              </button>
            ))}
          </div>
          <div className="mt-6" role="tabpanel">
            {tab === "Week" ? <WeekChart /> : null}
            {tab === "Reminders" ? (
              <ReminderEditor
                reminders={demo.reminders}
                onChange={demo.setReminders}
                onTest={(reminder) => demo.showToast(reminder.title || "Reminder", reminder.body || "Ping.")}
              />
            ) : null}
            {tab === "Settings" ? (
              <SettingsPanel
                idleSec={demo.idleSec}
                retainDays={demo.retainDays}
                recordTitles={demo.recordTitles}
                onIdle={demo.setIdleSec}
                onRetain={demo.setRetainDays}
                onTitles={demo.setRecordTitles}
              />
            ) : null}
          </div>
        </section>

        {demo.toast ? (
          <div
            role="status"
            className="fixed bottom-6 left-1/2 z-20 w-full max-w-sm -translate-x-1/2 border border-border bg-bg-elevated px-4 py-3"
          >
            <p className="font-display text-xl tracking-wide">{demo.toast.title}</p>
            <p className="mt-1 text-sm text-muted">{demo.toast.body}</p>
            <p className="mt-2 font-mono text-xs text-muted">Demo ping. No Windows toast.</p>
          </div>
        ) : null}
      </main>
    </SiteChrome>
  );
}

function resolvePlace(cityId: CityId, customLat: string, customLon: string) {
  if (cityId !== "custom") {
    return cities.find((city) => city.id === cityId) ?? cities[0];
  }
  const parsed = parseLatLon(customLat, customLon);
  if (!parsed) return null;
  const timeZone =
    typeof Intl !== "undefined" ? Intl.DateTimeFormat().resolvedOptions().timeZone : "UTC";
  return { ...parsed, timeZone, name: "Custom" };
}

function CityField({
  cityId,
  customLat,
  customLon,
  invalid,
  onCity,
  onCustom,
}: {
  cityId: CityId;
  customLat: string;
  customLon: string;
  invalid: boolean;
  onCity: (cityId: CityId) => void;
  onCustom: (lat: string, lon: string) => void;
}) {
  return (
    <div className="grid gap-2">
      <label className="grid gap-1 text-sm">
        <span className="font-mono text-xs text-muted">City</span>
        <select
          className={fieldClass}
          value={cityId}
          onChange={(event) => onCity(event.target.value as CityId)}
        >
          {cities.map((city) => (
            <option key={city.id} value={city.id}>
              {city.name}
            </option>
          ))}
          <option value="custom">Custom lat, lon</option>
        </select>
      </label>
      {cityId === "custom" ? (
        <div className="grid grid-cols-2 gap-2">
          <label className="grid gap-1 text-sm">
            <span className="font-mono text-xs text-muted">Lat</span>
            <input
              className={fieldClass}
              inputMode="decimal"
              value={customLat}
              onChange={(event) => onCustom(event.target.value, customLon)}
            />
          </label>
          <label className="grid gap-1 text-sm">
            <span className="font-mono text-xs text-muted">Lon</span>
            <input
              className={fieldClass}
              inputMode="decimal"
              value={customLon}
              onChange={(event) => onCustom(customLat, event.target.value)}
            />
          </label>
          {invalid ? <p className="col-span-2 text-sm text-danger">Use decimal degrees.</p> : null}
        </div>
      ) : null}
    </div>
  );
}
