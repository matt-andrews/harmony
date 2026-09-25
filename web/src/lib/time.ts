// Formatting and calendar math. All display is in the browser's local zone.

export function fmtClock(secs: number): string {
  const s = Math.max(0, Math.floor(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}:${String(r).padStart(2, '0')}`;
}

function fmtMinutes(mins: number): string {
  const h = Math.floor(mins / 60);
  const m = mins % 60;
  if (h === 0) return `${m}m`;
  return `${h}h ${String(m).padStart(2, '0')}m`;
}

/** "2h 08m", "45m", "0m" */
export function fmtDuration(secs: number): string {
  // Round to whole minutes first so 1h 59m 40s reads "2h 00m", not "1h 60m".
  return fmtMinutes(Math.round(Math.max(0, secs) / 60));
}

/**
 * A task's grand total, the figure that gets reported: whole minutes, always
 * rounded up. Only an exact :00 stays put (12:00:00 -> "12h 00m", 12:00:01 -> "12h 01m").
 */
export function fmtTaskTotal(secs: number): string {
  // Whole seconds first: the live clock is fractional, and 2:00.4 is still "on the 00".
  return fmtMinutes(Math.ceil(Math.floor(Math.max(0, secs)) / 60));
}

/** To the second, for individual sessions: "1h 02m 05s", "2m 05s", "5s" */
export function fmtExact(secs: number): string {
  const s = Math.floor(Math.max(0, secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = String(s % 60).padStart(2, '0');
  if (h > 0) return `${h}h ${String(m).padStart(2, '0')}m ${r}s`;
  if (m > 0) return `${m}m ${r}s`;
  return `${s % 60}s`;
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

const monthDayFmt = new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' });
/** "Sep 24", for badges where the year is noise. */
export function fmtMonthDay(d: Date | string): string {
  return monthDayFmt.format(typeof d === 'string' ? new Date(d) : d);
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

/** "YYYY-MM-DD" (a calendar date) -> local midnight. `new Date(str)` would read it as UTC. */
export function parseLocalDate(s: string | null | undefined): Date | null {
  if (!s) return null;
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
  if (!m) return null;
  const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  return isNaN(d.getTime()) ? null : d;
}

/** Local calendar date -> value for <input type="date">. */
export function toDateInput(d: Date | null): string {
  return d ? dayKey(d) : '';
}

// ----- reporting periods --------------------------------------------------

export type PeriodKind = 'week' | 'biweek' | 'month' | 'year';

export interface Period {
  kind: PeriodKind;
  from: Date;
  to: Date; // exclusive
  label: string;
}

/** What anchors the report calendar: a date a pay cycle starts on, or nothing (Monday weeks). */
export interface Cycle {
  start: Date | null;
}

function startOfDay(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

/** 00:00 of the most recent `weekStart` (a getDay() value, 0 = Sunday) on or before `d`. */
export function startOfWeek(d: Date, weekStart = 1): Date {
  const day = startOfDay(d);
  const dow = (day.getDay() - weekStart + 7) % 7;
  day.setDate(day.getDate() - dow);
  return day;
}

/** Calendar days, DST-safe (setDate keeps the local wall clock). */
export function addDays(d: Date, n: number): Date {
  const r = new Date(d);
  r.setDate(r.getDate() + n);
  return r;
}

/** Whole weeks from `a` to `b`, both week starts; DST makes the quotient inexact, so round. */
function weeksBetween(a: Date, b: Date): number {
  return Math.round((b.getTime() - a.getTime()) / (7 * 86400000));
}

const monthFmt = new Intl.DateTimeFormat(undefined, { month: 'long', year: 'numeric' });

/** Period `offset` steps away from the one containing `now` (0 = current, -1 = previous). */
export function period(kind: PeriodKind, offset: number, cycle: Cycle, now = new Date()): Period {
  const weekStart = cycle.start?.getDay() ?? 1;
  let from: Date;
  let to: Date;
  let label: string;
  switch (kind) {
    case 'week': {
      from = addDays(startOfWeek(now, weekStart), offset * 7);
      to = addDays(from, 7);
      label = `${monthDayFmt.format(from)} – ${monthDayFmt.format(addDays(to, -1))}`;
      break;
    }
    case 'biweek': {
      const anchor = cycle.start ?? startOfWeek(now, weekStart);
      const cycleIndex = Math.floor(weeksBetween(anchor, startOfWeek(now, weekStart)) / 2) + offset;
      from = addDays(anchor, cycleIndex * 14);
      to = addDays(from, 14);
      label = `${monthDayFmt.format(from)} – ${monthDayFmt.format(addDays(to, -1))}`;
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

/**
 * Tasks turned in inside this window become withdrawable inside `p`: the
 * period shifted back by the payout delay, in local calendar days.
 */
export function completionWindow(p: Period, delayDays: number): { from: Date; to: Date } {
  return { from: addDays(p.from, -delayDays), to: addDays(p.to, -delayDays) };
}
