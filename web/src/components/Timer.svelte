<script lang="ts">
  import {
    app,
    activeSession,
    elapsedSecs,
    livePay,
    startSession,
    stopSession,
    tagSession,
    taskTotalSecs,
    projectById,
  } from '../lib/state.svelte';
  import type { PickChoice } from '../lib/state.svelte';
  import { fmtClock, fmtMoney, fmtTaskTotal, fmtTime } from '../lib/time';
  import ProjectPicker from './ProjectPicker.svelte';

  const running = $derived(activeSession());
  const projects = $derived(app.view?.projects ?? []);

  // What the next Start does. By default it resumes the last project worked on
  // (the server derives that from the data); `override` is an explicit pick made
  // while idle, with `null` meaning "start untagged".
  let override = $state<PickChoice | null | undefined>(undefined);
  const nextChoice = $derived.by((): PickChoice | null => {
    if (override === null || override?.kind === 'create') return override;
    if (override?.kind === 'project') {
      // Ignore a pick that was archived from the Projects tab in the meantime.
      const p = projectById(override.projectId);
      if (p && !p.archived) return override;
    }
    const resume = app.view?.resume_project_id;
    return resume ? { kind: 'project', projectId: resume, newTask: false } : null;
  });
  const nextProject = $derived(nextChoice?.kind === 'project' ? projectById(nextChoice.projectId) : undefined);

  async function onStart(newTask = false) {
    const choice = nextChoice && nextChoice.kind !== 'untag' && newTask ? { ...nextChoice, newTask } : nextChoice;
    await startSession(choice);
    override = undefined;
  }

  async function onTagRunning(choice: PickChoice) {
    if (!running) return;
    await tagSession(running.id, choice);
  }

  const runningProject = $derived(running ? projectById(running.project_id) : undefined);
  const runningTaskTotal = $derived(running ? taskTotalSecs(running) : null);
  const nextLabel = $derived.by(() => {
    if (!nextChoice) return null;
    if (nextChoice.kind === 'create') return `will create “${nextChoice.name}”`;
    const p = nextProject;
    if (!p) return null;
    if (p.current_task_number === null) return 'task #1';
    // A turned-in task is closed: Start opens the next one by itself.
    if ((nextChoice.kind === 'project' && nextChoice.newTask) || p.current_task_completed_at) {
      return `task #${p.current_task_number + 1} (new)`;
    }
    return `task #${p.current_task_number} · ${fmtTaskTotal(p.current_task_total_secs ?? 0)} so far`;
  });
  /** "New task" only makes sense once the project has an open task to move on from. */
  const canStartNewTask = $derived(
    nextChoice?.kind === 'project' &&
      !nextChoice.newTask &&
      nextProject?.current_task_number != null &&
      nextProject.current_task_completed_at === null,
  );
</script>

<section class="timer card" class:running={!!running} style:--band={runningProject?.color ?? 'transparent'}>
  {#if running}
    <div class="readout">
      <div class="clock mono">{fmtClock(elapsedSecs(running))}</div>
      {#if runningTaskTotal !== null}
        <div class="total muted">
          task #{running.task_number} total <strong class="mono">{fmtTaskTotal(runningTaskTotal)}</strong>
        </div>
      {/if}
    </div>
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
          projectId={nextProject?.id ?? null}
          placeholder="Project for next session (optional)"
          onpick={(c) => {
            override = c.kind === 'untag' ? null : c;
          }}
        />
      </div>
    </div>
    <div class="actions">
      <button class="big primary" onclick={() => onStart()} disabled={app.busy}>Start</button>
      {#if canStartNewTask && nextProject}
        <button
          onclick={() => onStart(true)}
          disabled={app.busy}
          title="Start a fresh pickup of {nextProject.name}"
        >
          New task #{(nextProject.current_task_number ?? 0) + 1}
        </button>
      {/if}
    </div>
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
  .total {
    font-size: 14px;
  }
  .total strong {
    color: var(--text);
    font-size: 17px;
    margin-left: 4px;
  }
  .actions {
    display: flex;
    flex-direction: column;
    gap: 6px;
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
    color: var(--accent-text);
    font-weight: 600;
  }
  .big.danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--danger) 85%, white);
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
