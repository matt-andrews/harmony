<script lang="ts">
  import {
    app,
    activeSession,
    elapsedSecs,
    livePay,
    startSession,
    stopSession,
    tagSession,
    projectById,
  } from '../lib/state.svelte';
  import type { PickChoice } from '../lib/state.svelte';
  import { fmtClock, fmtMoney, fmtTime } from '../lib/time';
  import ProjectPicker from './ProjectPicker.svelte';

  const LAST_KEY = 'harmony.lastProject';

  const running = $derived(activeSession());
  const projects = $derived(app.view?.projects ?? []);

  // Project preselected for the next Start, remembered across reloads.
  let nextChoice = $state<PickChoice | null>(null);
  let nextProjectId = $derived.by(() => {
    if (nextChoice?.kind === 'project') return nextChoice.projectId;
    return null;
  });
  $effect(() => {
    if (nextChoice) return;
    try {
      const last = localStorage.getItem(LAST_KEY);
      if (last && projects.some((p) => p.id === last && !p.archived)) {
        nextChoice = { kind: 'project', projectId: last, newTask: false };
      }
    } catch {
      /* ignore */
    }
  });

  /** Persist and preselect the project for the next Start (never carrying "new task" over). */
  function remember(id: string | null) {
    nextChoice = id ? { kind: 'project', projectId: id, newTask: false } : null;
    try {
      if (id) localStorage.setItem(LAST_KEY, id);
      else localStorage.removeItem(LAST_KEY);
    } catch {
      /* ignore */
    }
  }

  async function onStart() {
    await startSession(nextChoice);
    remember(activeSession()?.project_id ?? null);
  }

  async function onTagRunning(choice: PickChoice) {
    if (!running) return;
    await tagSession(running.id, choice);
    remember(activeSession()?.project_id ?? null);
  }

  const runningProject = $derived(running ? projectById(running.project_id) : undefined);
  const nextLabel = $derived.by(() => {
    if (!nextChoice) return null;
    if (nextChoice.kind === 'create') return `will create “${nextChoice.name}”`;
    if (nextChoice.kind === 'untag') return null;
    const p = projectById(nextChoice.projectId);
    if (!p) return null;
    const n = p.current_task_number === null ? 1 : nextChoice.newTask ? p.current_task_number + 1 : p.current_task_number;
    return `task #${n}${nextChoice.newTask ? ' (new)' : ''}`;
  });
</script>

<section class="timer card" class:running={!!running} style:--band={runningProject?.color ?? 'transparent'}>
  {#if running}
    <div class="clock mono">{fmtClock(elapsedSecs(running))}</div>
    <div class="meta">
      <div class="row">
        <span class="muted">since {fmtTime(running.started_at)}</span>
        {#if running.task_number !== null}
          <span class="chip">task #{running.task_number} · session {running.ordinal}</span>
        {/if}
        {#if runningProject && runningProject.hourly_rate > 0}
          <span class="pay mono">{fmtMoney(livePay(running))}</span>
        {/if}
      </div>
      <div class="pick">
        <ProjectPicker {projects} projectId={running.project_id} placeholder="Tag this session…" onpick={onTagRunning} />
      </div>
    </div>
    <button class="big danger" onclick={stopSession} disabled={app.busy}>Stop</button>
  {:else}
    <div class="clock mono idle">00:00:00</div>
    <div class="meta">
      <div class="row muted">
        <span>Not working.</span>
        {#if nextLabel}<span class="chip">{nextLabel}</span>{/if}
      </div>
      <div class="pick">
        <ProjectPicker
          {projects}
          projectId={nextProjectId}
          placeholder="Project for next session (optional)"
          onpick={(c) => {
            if (c.kind === 'project') remember(c.projectId);
            else if (c.kind === 'untag') remember(null);
            else nextChoice = c;
          }}
        />
      </div>
    </div>
    <button class="big primary" onclick={onStart} disabled={app.busy}>Start</button>
  {/if}
</section>

<style>
  .timer {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 16px;
    align-items: center;
    padding: 16px 18px;
    margin-bottom: 18px;
    border-left: 4px solid var(--band);
  }
  .timer.running {
    border-color: var(--band);
  }
  .clock {
    font-size: 36px;
    font-weight: 600;
    letter-spacing: 0.02em;
    min-width: 170px;
  }
  .clock.idle {
    color: var(--muted);
  }
  .meta {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .pick {
    max-width: 360px;
  }
  .pay {
    color: var(--ok);
    font-weight: 600;
  }
  .big {
    font-size: 17px;
    padding: 12px 28px;
    border-radius: 10px;
    min-width: 110px;
  }
  .big.danger {
    background: var(--danger);
    border-color: var(--danger);
    color: white;
    font-weight: 600;
  }
  .big.danger:hover:not(:disabled) {
    background: #f05252;
  }
  @media (max-width: 560px) {
    .timer {
      grid-template-columns: 1fr auto;
    }
    .meta {
      grid-column: 1 / -1;
    }
    .clock {
      font-size: 30px;
      min-width: 0;
    }
  }
</style>
