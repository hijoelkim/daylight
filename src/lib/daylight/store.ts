import { create } from "zustand";
import { z } from "zod";
import {
  defaultReminders,
  reminderKinds,
  type CityId,
  type Reminder,
} from "@/lib/daylight/demo-data";

const STORAGE_KEY = "daylight-demo";

const reminderSchema = z.object({
  id: z.string(),
  title: z.string(),
  body: z.string(),
  kind: z.enum(reminderKinds),
  intervalMin: z.number(),
  timeLocal: z.string(),
  afterScreenMin: z.number(),
  enabled: z.boolean(),
});

const persistedSchema = z.object({
  cityId: z.enum(["sydney", "london", "new-york", "tokyo", "custom"]),
  customLat: z.string(),
  customLon: z.string(),
  cleared: z.boolean(),
  reminders: z.array(reminderSchema),
  idleSec: z.number().int().gte(30).lte(300),
  retainDays: z.number().int().gte(1).lte(3650),
  recordTitles: z.boolean(),
});

type Persisted = z.infer<typeof persistedSchema>;

export type DemoToast = { id: number; title: string; body: string };

type DemoState = Persisted & {
  ready: boolean;
  toast: DemoToast | null;
  hydrate: () => void;
  setCity: (cityId: CityId) => void;
  setCustom: (customLat: string, customLon: string) => void;
  setCleared: (cleared: boolean) => void;
  setReminders: (reminders: Reminder[]) => void;
  setIdleSec: (idleSec: number) => void;
  setRetainDays: (retainDays: number) => void;
  setRecordTitles: (recordTitles: boolean) => void;
  showToast: (title: string, body: string) => void;
  dismissToast: () => void;
};

const defaults: Persisted = {
  cityId: "sydney",
  customLat: "-33.8688",
  customLon: "151.2093",
  cleared: false,
  reminders: defaultReminders,
  idleSec: 60,
  retainDays: 90,
  recordTitles: false,
};

function readPersisted(): Persisted | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = persistedSchema.safeParse(JSON.parse(raw));
    return parsed.success ? parsed.data : null;
  } catch {
    return null;
  }
}

function writePersisted(state: Persisted) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
}

function pick(state: DemoState): Persisted {
  return {
    cityId: state.cityId,
    customLat: state.customLat,
    customLon: state.customLon,
    cleared: state.cleared,
    reminders: state.reminders,
    idleSec: state.idleSec,
    retainDays: state.retainDays,
    recordTitles: state.recordTitles,
  };
}

export const useDemo = create<DemoState>((set, get) => ({
  ...defaults,
  ready: false,
  toast: null,
  hydrate: () => {
    if (get().ready) return;
    const saved = readPersisted();
    set(saved ? { ...saved, ready: true } : { ready: true });
  },
  setCity: (cityId) => {
    set({ cityId });
    writePersisted(pick({ ...get(), cityId }));
  },
  setCustom: (customLat, customLon) => {
    set({ customLat, customLon });
    writePersisted(pick({ ...get(), customLat, customLon }));
  },
  setCleared: (cleared) => {
    set({ cleared });
    writePersisted(pick({ ...get(), cleared }));
  },
  setReminders: (reminders) => {
    set({ reminders });
    writePersisted(pick({ ...get(), reminders }));
  },
  setIdleSec: (idleSec) => {
    if (!Number.isFinite(idleSec)) return;
    const next = Math.min(300, Math.max(30, Math.round(idleSec)));
    set({ idleSec: next });
    writePersisted(pick({ ...get(), idleSec: next }));
  },
  setRetainDays: (retainDays) => {
    if (!Number.isFinite(retainDays)) return;
    const next = Math.min(3650, Math.max(1, Math.round(retainDays)));
    set({ retainDays: next });
    writePersisted(pick({ ...get(), retainDays: next }));
  },
  setRecordTitles: (recordTitles) => {
    set({ recordTitles });
    writePersisted(pick({ ...get(), recordTitles }));
  },
  showToast: (title, body) => set({ toast: { id: Date.now(), title, body } }),
  dismissToast: () => set({ toast: null }),
}));
