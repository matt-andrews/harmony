<script lang="ts">
  import * as api from '../lib/api';
  import type { Report } from '../lib/api';
  import { app } from '../lib/state.svelte';
  import { biweekAnchor, fmtDate, fmtHours, fmtMoney, period, setBiweekAnchor, toLocalInput, fromLocalInput } from '../lib/time';
  import type { PeriodKind } from '../lib/time';

  const kinds: { id: PeriodKind; label: string }[] = [
    { id: 'week', label: 'Week' },
    { id: 'biweek', label: '2 Weeks' },
    { id: 'month', label: 'Month' },
    { id: 'year', label: 'Year' },
  ];

  let kind = $state<PeriodKind>('week');
  let offset = $state(0);
  let anchorVersion = $state(0);
  const current = $derived.by(() => {
    void anchorVersion;
    return period(kind, offset);
  });

  let report = $state<Report | null>(null);
  let loading = $state(false);

  $effect(() => {
    const p = current;
    void app.view; // refetch when data changes
    loading = true;
    api
      .getReport(p.from, p.to)
      .then((r) => (report = r))
      .catch((e) => (app.error = e instanceof Error ? e.message : String(e)))
      .finally(() => (loading = false));
  });

  function pick(k: PeriodKind) {
    kind = k;
    offset = 0;
  }

  function onAnchorChange(e: Event) {
    const iso = fromLocalInput((e.target as HTMLInputElement).value);
    if (!iso) return;
    setBiweekAnchor(new Date(iso));
    offset = 0;
    anchorVersion++;
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

{#if kind === 'biweek'}
  <p class="muted small anchor">
    Cycle starts on
    <input type="datetime-local" value={toLocalInput(biweekAnchor().toISOString())} onchange={onAnchorChange} />
    (Mondays; pick any day in a cycle's first week)
  </p>
{/if}

<div class="card table" class:loading>
  {#if report}
    <table>
      <thead>
        <tr>
          <th>Project</th>
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
  .anchor {
    margin: -4px 0 12px;
  }
  .anchor input {
    margin: 0 6px;
  }
  .table {
    padding: 4px 6px;
    transition: opacity 0.15s;
  }
  .table.loading {
    opacity: 0.6;
  }
  .pay {
    color: var(--ok);
  }
  .range {
    padding: 6px 10px 4px;
    margin: 0;
  }
</style>
