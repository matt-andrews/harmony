<script lang="ts">
  import * as api from '../lib/api';
  import type { ProjectSummary, ProjectView } from '../lib/api';
  import { app, updateProject } from '../lib/state.svelte';
  import { fmtDate, fmtDuration, fmtMoney, fmtTaskTotal, fmtTime } from '../lib/time';

  const PALETTE = [
    '#f5a97f', '#8aadf4', '#a6da95', '#f5bde6', '#eed49f', '#c6a0f6',
    '#8bd5ca', '#ed8796', '#91d7e3', '#7dc4e4', '#b7bdf8', '#f0c6c6',
  ];

  let showArchived = $state(false);
  const projects = $derived((app.view?.projects ?? []).filter((p) => showArchived || !p.archived));
  const archivedCount = $derived((app.view?.projects ?? []).filter((p) => p.archived).length);

  let expanded = $state<Record<string, ProjectSummary | 'loading' | undefined>>({});

  async function toggle(p: ProjectView) {
    if (expanded[p.id]) {
      expanded[p.id] = undefined;
      return;
    }
    expanded[p.id] = 'loading';
    try {
      expanded[p.id] = await api.projectSummary(p.id);
    } catch (e) {
      app.error = e instanceof Error ? e.message : String(e);
      expanded[p.id] = undefined;
    }
  }

  // Refresh any open summaries when the state changes (rates, sessions...).
  $effect(() => {
    void app.view;
    for (const id of Object.keys(expanded)) {
      if (expanded[id] && expanded[id] !== 'loading') {
        void api.projectSummary(id).then((s) => (expanded[id] = s)).catch(() => {});
      }
    }
  });

  function cycleColor(p: ProjectView) {
    const i = PALETTE.indexOf(p.color.toLowerCase());
    const next = PALETTE[(i + 1) % PALETTE.length];
    void updateProject(p.id, { color: next });
  }

  function saveRate(p: ProjectView, e: Event) {
    const v = parseFloat((e.target as HTMLInputElement).value);
    if (!isNaN(v) && v !== p.hourly_rate) void updateProject(p.id, { hourly_rate: v });
  }

  function saveName(p: ProjectView, e: Event) {
    const v = (e.target as HTMLInputElement).value.trim();
    if (v && v !== p.name) void updateProject(p.id, { name: v });
    else (e.target as HTMLInputElement).value = p.name;
  }

  function blurOnEnter(e: KeyboardEvent) {
    if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
  }
</script>

<div class="row head">
  <h2>Projects</h2>
  <span class="grow"></span>
  {#if archivedCount > 0}
    <label class="row muted small">
      <input type="checkbox" bind:checked={showArchived} />
      show {archivedCount} archived
    </label>
  {/if}
</div>

{#if projects.length === 0}
  <p class="muted empty">No projects yet. Tag a session to create one.</p>
{/if}

{#each projects as p (p.id)}
  <article class="project card" class:archived={p.archived} style:--band={p.color}>
    <div class="band"></div>
    <div class="body">
      <div class="row top">
        <button class="swatch" style:background={p.color} onclick={() => cycleColor(p)} title="Change colour" aria-label="Change colour"></button>
        <input class="name" type="text" value={p.name} onblur={(e) => saveName(p, e)} onkeydown={blurOnEnter} />
        <label class="rate row">
          <span class="muted">$</span>
          <input type="number" min="0" step="0.5" value={p.hourly_rate} onblur={(e) => saveRate(p, e)} onkeydown={blurOnEnter} />
          <span class="muted">/h</span>
        </label>
      </div>
      <div class="row stats">
        <span class="mono strong">{fmtDuration(p.total_secs)}</span>
        <span class="muted">·</span>
        <span class="mono pay">{fmtMoney(p.total_pay)}</span>
        <span class="muted">·</span>
        <span class="muted">{p.task_count} {p.task_count === 1 ? 'task' : 'tasks'}</span>
        {#if p.current_task_number !== null}
          <span class="chip">current: task #{p.current_task_number}</span>
        {/if}
        <span class="grow"></span>
        <button class="ghost" onclick={() => toggle(p)}>{expanded[p.id] ? 'Hide tasks' : 'Tasks'}</button>
        <button class="ghost" onclick={() => updateProject(p.id, { archived: !p.archived })}>
          {p.archived ? 'Unarchive' : 'Archive'}
        </button>
      </div>

      {#if expanded[p.id] === 'loading'}
        <p class="muted small">Loading…</p>
      {:else if expanded[p.id]}
        {@const summary = expanded[p.id] as ProjectSummary}
        <table class="tasks">
          <thead>
            <tr><th>Task</th><th>Started</th><th>Last worked</th><th class="num">Sessions</th><th class="num">Time</th><th class="num">Pay</th></tr>
          </thead>
          <tbody>
            {#each summary.tasks as t (t.id)}
              <tr>
                <td>#{t.number}</td>
                <td>{t.first_started ? `${fmtDate(t.first_started)} ${fmtTime(t.first_started)}` : '—'}</td>
                <td>{t.last_ended ? `${fmtDate(t.last_ended)} ${fmtTime(t.last_ended)}` : 'running'}</td>
                <td class="num">{t.session_count}</td>
                <td class="num">{fmtTaskTotal(t.total_secs)}</td>
                <td class="num">{fmtMoney(t.total_pay)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  </article>
{/each}

<style>
  .head {
    margin-bottom: 10px;
  }
  h2 {
    font-size: 16px;
  }
  .small {
    font-size: 13px;
  }
  .empty {
    text-align: center;
    padding: 40px 0;
  }
  .project {
    display: flex;
    margin-bottom: 8px;
  }
  .project.archived {
    opacity: 0.55;
  }
  .band {
    width: 5px;
    flex-shrink: 0;
    background: var(--band);
    border-radius: var(--radius) 0 0 var(--radius);
  }
  .body {
    flex: 1;
    min-width: 0;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .swatch {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    padding: 0;
    border: 2px solid var(--surface);
    box-shadow: 0 0 0 1px var(--border);
    flex-shrink: 0;
  }
  .name {
    flex: 1;
    font-weight: 600;
    background: transparent;
    border-color: transparent;
  }
  .name:hover,
  .name:focus {
    border-color: var(--border);
    background: var(--bg);
  }
  .rate input {
    width: 80px;
  }
  .stats {
    flex-wrap: wrap;
    font-size: 14px;
  }
  .strong {
    font-weight: 600;
  }
  .pay {
    color: var(--ok);
  }
  .tasks {
    font-size: 13.5px;
    margin-top: 4px;
  }
</style>
