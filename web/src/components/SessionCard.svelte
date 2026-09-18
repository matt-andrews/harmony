<script lang="ts">
  import type { SessionView, UpdateSession } from '../lib/api';
  import { app, elapsedSecs, livePay, taskTotalSecs, tagSession, updateSession, deleteSession } from '../lib/state.svelte';
  import { fmtExact, fmtMoney, fmtTaskTotal, fmtTime, fromLocalInput, toLocalInput } from '../lib/time';
  import ProjectPicker from './ProjectPicker.svelte';

  interface Props {
    session: SessionView;
  }
  let { session }: Props = $props();

  const projects = $derived(app.view?.projects ?? []);
  const active = $derived(session.ended_at === null);
  const isFirst = $derived(session.ordinal === 1);

  let editing = $state(false);
  let startInput = $state('');
  let endInput = $state('');
  let noteInput = $state('');

  function beginEdit() {
    startInput = toLocalInput(session.started_at);
    endInput = toLocalInput(session.ended_at);
    noteInput = session.note ?? '';
    editing = true;
  }

  async function save() {
    // The inputs are minute-precision, so only send a time the user actually
    // changed; otherwise an unrelated note edit would truncate the seconds.
    const patch: UpdateSession = { note: noteInput.trim() || null };
    if (startInput !== toLocalInput(session.started_at)) {
      const started_at = fromLocalInput(startInput);
      if (!started_at) {
        app.error = 'Start time is required';
        return;
      }
      patch.started_at = started_at;
    }
    if (endInput !== toLocalInput(session.ended_at)) {
      patch.ended_at = fromLocalInput(endInput);
    }
    const ok = await updateSession(session.id, patch);
    if (ok) editing = false;
  }

  async function remove() {
    if (!confirm('Delete this session? This cannot be undone.')) return;
    await deleteSession(session.id);
  }
</script>

<article class="session card" class:active style:--band={session.color ?? 'var(--border)'}>
  <div class="band"></div>
  <div class="body">
    <header class="row">
      <div class="pick">
        <ProjectPicker {projects} projectId={session.project_id} placeholder="Untagged — pick a project" onpick={(c) => tagSession(session.id, c)} />
      </div>
      {#if session.task_number !== null}
        <span class="chip">task #{session.task_number}</span>
        <span class="chip" class:first={isFirst}>
          session {session.ordinal} of {session.task_session_count}
          {#if (session.task_session_count ?? 0) > 1}· task {fmtTaskTotal(taskTotalSecs(session) ?? 0)}{/if}
        </span>
        {#if isFirst}
          <span class="badge">new task</span>
        {/if}
      {/if}
      <span class="grow"></span>
      <button class="ghost" onclick={editing ? () => (editing = false) : beginEdit} aria-label="Edit times">
        {editing ? 'Cancel' : 'Edit'}
      </button>
      <button class="ghost danger" onclick={remove} aria-label="Delete session">Delete</button>
    </header>

    <div class="row times">
      <span class="mono">
        {fmtTime(session.started_at)} – {session.ended_at ? fmtTime(session.ended_at) : 'now'}
      </span>
      <span class="muted">·</span>
      <span class="mono strong">{fmtExact(elapsedSecs(session))}</span>
      {#if session.project_id}
        <span class="muted">·</span>
        <span class="mono pay">{fmtMoney(livePay(session))}</span>
      {/if}
      {#if session.note && !editing}
        <span class="muted note">— {session.note}</span>
      {/if}
    </div>

    {#if editing}
      <div class="edit">
        <label>
          <span class="muted">Start</span>
          <input type="datetime-local" bind:value={startInput} />
        </label>
        <label>
          <span class="muted">End</span>
          <input type="datetime-local" bind:value={endInput} />
        </label>
        <label class="grow">
          <span class="muted">Note</span>
          <input type="text" bind:value={noteInput} placeholder="optional" />
        </label>
        <button class="primary" onclick={save} disabled={app.busy}>Save</button>
      </div>
    {/if}
  </div>
</article>

<style>
  .session {
    display: flex;
    overflow: visible;
    margin-bottom: 8px;
  }
  .session.active {
    border-color: var(--band);
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
  header {
    flex-wrap: wrap;
  }
  .pick {
    width: 220px;
    max-width: 100%;
  }
  .chip.first {
    border-color: var(--band);
  }
  .badge {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--band);
    border: 1px solid var(--band);
    border-radius: 4px;
    padding: 1px 6px;
  }
  .times {
    flex-wrap: wrap;
    font-size: 14px;
  }
  .strong {
    font-weight: 600;
  }
  .pay {
    color: var(--ok);
  }
  .note {
    font-style: italic;
  }
  .edit {
    display: flex;
    gap: 10px;
    align-items: end;
    flex-wrap: wrap;
    padding-top: 6px;
    border-top: 1px dashed var(--border);
  }
  .edit label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
  }
</style>
