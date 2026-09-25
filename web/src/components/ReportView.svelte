<script lang="ts">
  import * as api from '../lib/api';
  import type { Report } from '../lib/api';
  import { app, updateSettings } from '../lib/state.svelte';
  import {
    addDays,
    completionWindow,
    fmtDate,
    fmtHours,
    fmtMoney,
    fmtTaskTotal,
    parseLocalDate,
    period,
    toDateInput,
  } from '../lib/time';
  import type { PeriodKind } from '../lib/time';

  const kinds: { id: PeriodKind; label: string }[] = [
    { id: 'week', label: 'Week' },
    { id: 'biweek', label: '2 Weeks' },
    { id: 'month', label: 'Month' },
    { id: 'year', label: 'Year' },
  ];

  let kind = $state<PeriodKind>('biweek');
  let offset = $state(0);

  // Settings travel with the data, so they follow the same store as everything else.
  const settings = $derived(app.view?.settings ?? { cycle_start: null, payout_delay_days: 7 });
  const cycleStart = $derived(parseLocalDate(settings.cycle_start));
  const delay = $derived(settings.payout_delay_days);
  const current = $derived(period(kind, offset, { start: cycleStart }));
  const doneWindow = $derived(completionWindow(current, delay));
  const payday = $derived(addDays(current.to, -1));
  const inProgress = $derived(current.to.getTime() > app.now);

  let report = $state<Report | null>(null);
  let loading = $state(false);

  $effect(() => {
    const p = current;
    const w = doneWindow;
    void app.view; // refetch when data changes
    loading = true;
    api
      .getReport(p.from, p.to, w)
      .then((r) => (report = r))
      .catch((e) => (app.error = e instanceof Error ? e.message : String(e)))
      .finally(() => (loading = false));
  });

  function pick(k: PeriodKind) {
    kind = k;
    offset = 0;
  }

  async function onCycleStart(e: Event) {
    const value = (e.target as HTMLInputElement).value;
    if (value && !parseLocalDate(value)) return;
    if (await updateSettings({ cycle_start: value || null })) offset = 0;
  }

  async function onDelay(e: Event) {
    const input = e.target as HTMLInputElement;
    const v = parseInt(input.value, 10);
    if (isNaN(v) || v < 0 || v > 90) {
      input.value = String(delay);
      return;
    }
    if (v !== delay) await updateSettings({ payout_delay_days: v });
  }
</script>

<div class="controls">
  <div class="row kinds">
    {#each kinds as k (k.id)}
      <button class:active={kind === k.id} onclick={() => pick(k.id)}>{k.label}</button>
    {/each}
  </div>
  <div class="row nav">
    <button class="ghost" onclick={() => offset--} aria-label="Previous">‹</button>
    <span class="label">{current.label}</span>
    <button class="ghost" onclick={() => offset++} disabled={offset >= 0} aria-label="Next">›</button>
    {#if offset !== 0}
      <button class="ghost" onclick={() => (offset = 0)}>Now</button>
    {/if}
  </div>
</div>

<p class="muted small cycle row">
  <label class="row">
    Cycle starts
    <input type="date" value={toDateInput(cycleStart)} onchange={onCycleStart} />
  </label>
  <span class="sep">·</span>
  <label class="row">
    pay finalizes
    <input type="number" min="0" max="90" step="1" value={delay} onchange={onDelay} />
    days after a task is done
  </label>
</p>

<div class="card table" class:loading>
  {#if report}
    <table>
      <thead>
        <tr>
          <th>Worked</th>
          <th class="num">Sessions</th>
          <th class="num">Hours</th>
          <th class="num">Pay</th>
        </tr>
      </thead>
      <tbody>
        {#if report.rows.length === 0}
          <tr><td colspan="4" class="muted">Nothing tracked in this period.</td></tr>
        {/if}
        {#each report.rows as r (r.project_id ?? 'untagged')}
          <tr>
            <td>
              <span class="row">
                <span class="dot" style:background={r.color ?? 'var(--muted)'}></span>
                <span class:muted={!r.project_id}>{r.project_name}</span>
              </span>
            </td>
            <td class="num">{r.session_count}</td>
            <td class="num">{fmtHours(r.total_secs)}</td>
            <td class="num">{r.project_id ? fmtMoney(r.total_pay) : '—'}</td>
          </tr>
        {/each}
      </tbody>
      <tfoot>
        <tr>
          <td>Total</td>
          <td class="num"></td>
          <td class="num">{fmtHours(report.total_secs)}</td>
          <td class="num pay">{fmtMoney(report.total_pay)}</td>
        </tr>
      </tfoot>
    </table>
    <p class="muted small range">
      {fmtDate(report.from)} → {fmtDate(new Date(Date.parse(report.to) - 1))}, inclusive. Running sessions count up to now.
    </p>
  {:else}
    <p class="muted" style="padding: 12px">Loading…</p>
  {/if}
</div>

<div class="card table payout" class:loading>
  {#if report}
    <div class="row head">
      <h3>Payout</h3>
      <span class="muted small">
        available by {fmtDate(payday)}{inProgress ? ' (so far)' : ''} · tasks done {fmtDate(doneWindow.from)} – {fmtDate(addDays(doneWindow.to, -1))}
      </span>
    </div>
    <table>
      <thead>
        <tr>
          <th>Project</th>
          <th>Task</th>
          <th>Done</th>
          <th class="num">Time</th>
          <th class="num">Pay</th>
        </tr>
      </thead>
      <tbody>
        {#if report.payout_rows.length === 0}
          <tr><td colspan="5" class="muted">No tasks were turned in during that window.</td></tr>
        {/if}
        {#each report.payout_rows as r (r.task_id)}
          <tr>
            <td>
              <span class="row">
                <span class="dot" style:background={r.color}></span>
                <span>{r.project_name}</span>
              </span>
            </td>
            <td>#{r.task_number}</td>
            <td>{fmtDate(r.completed_at)}</td>
            <td class="num">{fmtTaskTotal(r.total_secs)}</td>
            <td class="num">{fmtMoney(r.total_pay)}</td>
          </tr>
        {/each}
      </tbody>
      <tfoot>
        <tr>
          <td>Withdrawable</td>
          <td></td>
          <td></td>
          <td class="num"></td>
          <td class="num pay">{fmtMoney(report.payout_total_pay)}</td>
        </tr>
      </tfoot>
    </table>
    {#if report.carried_rows.length > 0}
      <p class="muted small range">
        Carried to the next payout: {report.carried_rows.length}
        {report.carried_rows.length === 1 ? 'task' : 'tasks'} done after {fmtDate(addDays(doneWindow.to, -1))} ·
        <span class="mono">{fmtMoney(report.carried_total_pay)}</span>
      </p>
    {/if}
  {/if}
</div>

<style>
  .controls {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 12px;
  }
  .kinds button.active {
    background: var(--surface-3);
    border-color: var(--accent);
  }
  .label {
    min-width: 150px;
    text-align: center;
    font-weight: 600;
  }
  .small {
    font-size: 13px;
  }
  .cycle {
    margin: -4px 0 12px;
    flex-wrap: wrap;
    row-gap: 4px;
  }
  .cycle label {
    gap: 6px;
    cursor: default;
  }
  .cycle input {
    font-size: 13px;
    padding: 3px 8px;
  }
  .cycle input[type='number'] {
    width: 58px;
  }
  .sep {
    margin: 0 2px;
  }
  .table {
    padding: 4px 6px;
    transition: opacity 0.15s;
  }
  .table.loading {
    opacity: 0.6;
  }
  .payout {
    margin-top: 12px;
  }
  .head {
    padding: 8px 10px 2px;
    flex-wrap: wrap;
  }
  h3 {
    font-size: 15px;
  }
  .pay {
    color: var(--ok);
  }
  .range {
    padding: 6px 10px 4px;
    margin: 0;
  }
</style>
