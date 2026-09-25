<script lang="ts">
  import {
    app,
    activeSession,
    dismissError,
    elapsedSecs,
    projectById,
    startSession,
    stopSession,
    taskTotalSecs,
  } from '../lib/state.svelte';
  import { exitCompact, startDrag } from '../lib/desktop.svelte';
  import { fmtClock, fmtTaskTotal } from '../lib/time';

  const running = $derived(activeSession());
  const runningTaskTotal = $derived(running ? taskTotalSecs(running) : null);
  // Idle, Start resumes the last project worked on, like the full timer's default.
  const project = $derived(projectById(running ? running.project_id : app.view?.resume_project_id));

  const detail = $derived.by(() => {
    if (running) return running.task_number === null ? null : `task #${running.task_number}`;
    if (!project) return null;
    if (project.current_task_number === null) return 'task #1';
    if (project.current_task_completed_at) return `task #${project.current_task_number + 1} (new)`;
    return `task #${project.current_task_number} · ${fmtTaskTotal(project.current_task_total_secs ?? 0)} so far`;
  });

  async function onStart() {
    await startSession(project ? { kind: 'project', projectId: project.id, newTask: false } : null);
  }

  // The strip has no title bar: anywhere that isn't a button drags the window.
  function onMouseDown(e: MouseEvent) {
    if (e.button !== 0 || (e.target as HTMLElement).closest('button')) return;
    startDrag();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<section class="strip" class:running={!!running} style:--band={project?.color ?? 'var(--border)'} onmousedown={onMouseDown}>
  <div class="info">
    {#if app.error}
      <button class="line error" title={app.error} onclick={dismissError}>{app.error}</button>
    {:else}
      <div class="line">
        <span class="name">{project?.name ?? (running ? 'Untagged' : 'No project')}</span>
        {#if detail}<span class="muted">{detail}</span>{/if}
      </div>
    {/if}
    <div class="times">
      <span class="clock mono" class:idle={!running}>{fmtClock(running ? elapsedSecs(running) : 0)}</span>
      {#if runningTaskTotal !== null}
        <span class="muted">total <strong class="mono">{fmtTaskTotal(runningTaskTotal)}</strong></span>
      {/if}
    </div>
  </div>
  {#if running}
    <button class="go stop" onclick={stopSession} disabled={app.busy}>Stop</button>
  {:else}
    <button class="go primary" onclick={onStart} disabled={app.busy || !app.loaded}>Start</button>
  {/if}
  <button class="ghost expand" onclick={exitCompact} title="Back to the full window" aria-label="Expand">⤢</button>
</section>

<style>
  .strip {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 100vh;
    padding: 0 6px 0 12px;
    background: var(--surface);
    border-left: 5px solid var(--band);
    user-select: none;
    cursor: grab;
  }
  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .line {
    display: flex;
    gap: 8px;
    font-size: 12.5px;
    white-space: nowrap;
    overflow: hidden;
  }
  .line .name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .line.error {
    display: block;
    padding: 0;
    border: none;
    background: none;
    color: var(--danger);
    text-align: left;
    text-overflow: ellipsis;
  }
  .times {
    display: flex;
    align-items: baseline;
    gap: 10px;
    font-size: 12.5px;
    white-space: nowrap;
  }
  .clock {
    font-size: 24px;
    font-weight: 600;
    line-height: 1.15;
    letter-spacing: 0.02em;
  }
  .clock.idle {
    color: var(--muted);
  }
  .times strong {
    color: var(--text);
    font-size: 14px;
  }
  .go {
    font-weight: 600;
    padding: 9px 18px;
    min-width: 76px;
  }
  .go.stop {
    background: var(--danger);
    border-color: var(--danger);
    color: var(--accent-text);
  }
  .go.stop:hover:not(:disabled) {
    background: color-mix(in srgb, var(--danger) 85%, white);
  }
  .expand {
    align-self: flex-start;
    margin-top: 4px;
    padding: 0 5px;
    font-size: 15px;
  }
</style>
