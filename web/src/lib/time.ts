// Formatting and calendar math. All display is in the browser's local zone.

export function fmtClock(secs: number): string {
  const s = Math.max(0, Math.floor(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}:${String(r).padStart(2, '0')}`;
}

/** "2h 08m", "45m", "0m" */
export function fmtDuration(secs: number): string {
  const s = Math.max(0, Math.round(secs));
  const h = Math.floor(s / 3600);
  const m = Math.round((s % 3600) / 60);
  if (h === 0) return `${m}m`;
  return `${h}h ${String(m).padStart(2, '0')}m`;
}

/** Decimal hours for reports: "12.75" */
export function fmtHours(secs: number): string {
  return (secs / 3600).toFixed(2);
}

const money = new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' });
export function fmtMoney(n: number): string {
  return money.format(n);
}

const timeFmt = new Intl.DateTimeFormat(undefined, { hour: 'numeric', minute: '2-digit' });
export function fmtTime(d: Date | string): string {
  return timeFmt.format(typeof d === 'string' ? new Date(d) : d);
}

const dayFmt = new Intl.DateTimeFormat(undefined, {
  weekday: 'short',
  month: 'short',
  day: 'numeric',
});
export function fmtDay(d: Date | string): string {
  const date = typeof d === 'string' ? new Date(d) : d;
  const today = new Date();
  const yesterday = new Date();
  yesterday.setDate(today.getDate() - 1);
  if (sameDay(date, today)) return `Today · ${dayFmt.format(date)}`;
  if (sameDay(date, yesterday)) return `Yesterday · ${dayFmt.format(date)}`;
  return dayFmt.format(date);
}

const dateFmt = new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric', year: 'numeric' });
export function fmtDate(d: Date | string): string {
  return dateFmt.format(typeof d === 'string' ? new Date(d) : d);
}

export function sameDay(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
}

/** Local calendar-day key for grouping: "2026-09-16" */
export function dayKey(d: Date | string): string {
  const date = typeof d === 'string' ? new Date(d) : d;
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${y}-${m}-${day}`;
}

/** ISO -> value for <input type="datetime-local"> (local time, minute precision). */
export function toLocalInput(iso: string | null): string {
  if (!iso) return '';
  const d = new Date(iso);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** datetime-local value -> ISO string, or null when blank. */
export function fromLocalInput(value: string): string | null {
  if (!value) return null;
  const d = new Date(value);
  return isNaN(d.getTime()) ? null : d.toISOString();
}

// ----- reporting periods --------------------------------------------------

export type PeriodKind = 'week' | 'biweek' | 'month' | 'year';

export interface Period {
  kind: PeriodKind;
  from: Date;
  to: Date; // exclusive
  label: string;
}

function startOfDay(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

/** Monday 00:00 of the week containing `d`. */
export function startOfWeek(d: Date): Date {
  const day = startOfDay(d);
  const dow = (day.getDay() + 6) % 7; // Mon=0 .. Sun=6
  day.setDate(day.getDate() - dow);
  return day;
}

function addDays(d: Date, n: number): Date {
  const r = new Date(d);
  r.setDate(r.getDate() + n);
  return r;
}

const BIWEEK_ANCHOR_KEY = 'harmony.biweekAnchor';

/** The Monday that starts a biweekly cycle. Defaults to this week's Monday. */
export function biweekAnchor(): Date {
  try {
    const stored = localStorage.getItem(BIWEEK_ANCHOR_KEY);
    if (stored) {
      const d = new Date(stored);
      if (!isNaN(d.getTime())) return startOfWeek(d);
    }
  } catch {
    /* storage unavailable */
  }
  return startOfWeek(new Date());
}

export function setBiweekAnchor(d: Date): void {
  try {
    localStorage.setItem(BIWEEK_ANCHOR_KEY, startOfWeek(d).toISOString());
  } catch {
    /* ignore */
  }
}

const rangeFmt = new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' });
const monthFmt = new Intl.DateTimeFormat(undefined, { month: 'long', year: 'numeric' });

/** Period `offset` steps away from the one containing `now` (0 = current, -1 = previous). */
export function period(kind: PeriodKind, offset: number, now = new Date()): Period {
  let from: Date;
  let to: Date;
  let label: string;
  switch (kind) {
    case 'week': {
      from = addDays(startOfWeek(now), offset * 7);
      to = addDays(from, 7);
      label = `${rangeFmt.format(from)} – ${rangeFmt.format(addDays(to, -1))}`;
      break;
    }
    case 'biweek': {
      const anchor = biweekAnchor();
      const weeksSince = Math.floor((startOfWeek(now).getTime() - anchor.getTime()) / (7 * 86400000));
      const cycle = Math.floor(weeksSince / 2) + offset;
      from = addDays(anchor, cycle * 14);
      to = addDays(from, 14);
      label = `${rangeFmt.format(from)} – ${rangeFmt.format(addDays(to, -1))}`;
      break;
    }
    case 'month': {
      from = new Date(now.getFullYear(), now.getMonth() + offset, 1);
      to = new Date(from.getFullYear(), from.getMonth() + 1, 1);
      label = monthFmt.format(from);
      break;
    }
    case 'year': {
      from = new Date(now.getFullYear() + offset, 0, 1);
      to = new Date(from.getFullYear() + 1, 0, 1);
      label = String(from.getFullYear());
      break;
    }
  }
  return { kind, from, to, label };
}
