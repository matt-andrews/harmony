<script lang="ts">
  // Combobox: filter existing projects, or create one when nothing matches.
  // Emits a PickChoice; the parent decides what to do with it.
  import type { ProjectView } from '../lib/api';
  import type { PickChoice } from '../lib/state.svelte';

  interface Props {
    projects: ProjectView[];
    /** Currently selected project (for display), if any. */
    projectId?: string | null;
    /** Show the "Untag" row when something is selected. */
    allowUntag?: boolean;
    placeholder?: string;
    onpick: (choice: PickChoice) => void | Promise<void>;
  }

  let { projects, projectId = null, allowUntag = true, placeholder = 'Project…', onpick }: Props = $props();

  let text = $state('');
  let open = $state(false);
  let highlighted = $state(0);
  let newTask = $state(false);
  let input: HTMLInputElement | undefined = $state();

  const selected = $derived(projects.find((p) => p.id === projectId) ?? null);
  const visible = $derived(projects.filter((p) => !p.archived || p.id === projectId));
  const query = $derived(text.trim().toLowerCase());
  const matches = $derived(
    query ? visible.filter((p) => p.name.toLowerCase().includes(query)) : visible,
  );
  const exact = $derived(matches.some((p) => p.name.toLowerCase() === query));
  const canCreate = $derived(query.length > 0 && !exact);

  type Row =
    | { kind: 'create'; label: string }
    | { kind: 'project'; project: ProjectView }
    | { kind: 'untag' };
  const rows = $derived.by((): Row[] => {
    const out: Row[] = [];
    if (canCreate) out.push({ kind: 'create', label: text.trim() });
    for (const p of matches) out.push({ kind: 'project', project: p });
    if (allowUntag && selected) out.push({ kind: 'untag' });
    return out;
  });

  $effect(() => {
    // Keep the highlight in range as the list changes.
    if (highlighted >= rows.length) highlighted = Math.max(0, rows.length - 1);
  });

  function show() {
    open = true;
    highlighted = 0;
  }

  function hide() {
    open = false;
    text = '';
  }

  async function choose(row: Row | undefined) {
    if (!row) return;
    let choice: PickChoice;
    switch (row.kind) {
      case 'create':
        choice = { kind: 'create', name: row.label, newTask };
        break;
      case 'project':
        choice = { kind: 'project', projectId: row.project.id, newTask };
        break;
      case 'untag':
        choice = { kind: 'untag' };
        break;
    }
    hide();
    newTask = false;
    input?.blur();
    await onpick(choice);
  }

  function onkeydown(e: KeyboardEvent) {
    if (!open && (e.key === 'ArrowDown' || e.key === 'Enter')) {
      show();
      e.preventDefault();
      return;
    }
    switch (e.key) {
      case 'ArrowDown':
        highlighted = Math.min(rows.length - 1, highlighted + 1);
        e.preventDefault();
        break;
      case 'ArrowUp':
        highlighted = Math.max(0, highlighted - 1);
        e.preventDefault();
        break;
      case 'Enter':
        void choose(rows[highlighted]);
        e.preventDefault();
        break;
      case 'Escape':
        hide();
        input?.blur();
        break;
    }
  }

  function onfocusout(e: FocusEvent) {
    const next = e.relatedTarget as Node | null;
    if (!next || !(e.currentTarget as HTMLElement).contains(next)) hide();
  }

  /** Task number a project will get if picked now. */
  function nextTaskLabel(p: ProjectView): string {
    const cur = p.current_task_number;
    if (cur === null) return 'task #1';
    // A turned-in task is closed, so picking the project starts the next one.
    if (newTask || p.current_task_completed_at) return `→ task #${cur + 1}`;
    return `task #${cur}`;
  }
</script>

<div class="picker" onfocusout={onfocusout}>
  <div class="field" class:open>
    {#if selected && !open}
      <span class="dot" style:background={selected.color}></span>
    {/if}
    <input
      bind:this={input}
      type="text"
      bind:value={text}
      placeholder={selected && !open ? selected.name : placeholder}
      class:has-selection={selected && !open}
      onfocus={show}
      oninput={() => { if (!open) show(); highlighted = 0; }}
      onkeydown={onkeydown}
      role="combobox"
      aria-expanded={open}
      aria-controls="picker-list"
      aria-autocomplete="list"
      autocomplete="off"
      spellcheck="false"
    />
  </div>
  {#if open}
    <!-- mousedown is swallowed so clicks inside the menu never blur the input -->
    <div class="menu card" id="picker-list" role="listbox" tabindex="-1" onmousedown={(e) => e.preventDefault()}>
      <label class="newtask">
        <input type="checkbox" bind:checked={newTask} />
        <span>New task <span class="muted">(fresh pickup of this project)</span></span>
      </label>
      {#if rows.length === 0}
        <div class="empty muted">Type a name to create a project</div>
      {/if}
      {#each rows as row, i (row.kind === 'project' ? row.project.id : row.kind)}
        <button
          type="button"
          class="item"
          class:hl={i === highlighted}
          role="option"
          aria-selected={i === highlighted}
          onmouseenter={() => (highlighted = i)}
          onclick={() => choose(row)}
        >
          {#if row.kind === 'create'}
            <span class="plus">＋</span>
            <span class="grow">Create <strong>“{row.label}”</strong></span>
            <span class="muted small">task #1</span>
          {:else if row.kind === 'project'}
            <span class="dot" style:background={row.project.color}></span>
            <span class="grow">{row.project.name}</span>
            <span class="muted small">{nextTaskLabel(row.project)}</span>
          {:else}
            <span class="plus">✕</span>
            <span class="grow muted">Untag</span>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .picker {
    position: relative;
    min-width: 0;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0 10px;
  }
  .field:focus-within {
    border-color: var(--accent);
  }
  .field input {
    border: none;
    background: transparent;
    padding: 6px 0;
    flex: 1;
    width: 100%;
  }
  .field input.has-selection::placeholder {
    color: var(--text);
  }
  .menu {
    position: absolute;
    z-index: 20;
    top: calc(100% + 4px);
    left: 0;
    min-width: 100%;
    width: max-content;
    max-width: min(420px, 90vw);
    padding: 4px;
    max-height: 320px;
    overflow-y: auto;
  }
  .newtask {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px 8px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 4px;
    font-size: 13px;
    cursor: pointer;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    text-align: left;
    border: none;
    background: transparent;
    padding: 7px 8px;
    border-radius: 6px;
  }
  .item.hl {
    background: var(--surface-3);
  }
  .plus {
    width: 10px;
    color: var(--accent);
    text-align: center;
  }
  .small {
    font-size: 12px;
  }
  .empty {
    padding: 8px;
    font-size: 13px;
  }
</style>
