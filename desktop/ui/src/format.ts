export function formatMs(ms: number): string {
  const total = Math.max(0, Math.round(ms / 60_000));
  const hours = Math.floor(total / 60);
  const minutes = total % 60;
  return `${hours}h ${String(minutes).padStart(2, "0")}m`;
}

export const emptyLine = "Leave it in the tray. Come back this evening.";
