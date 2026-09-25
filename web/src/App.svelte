<script lang="ts">
  import { onMount } from 'svelte';
  import { app, refresh, startClock, dismissError } from './lib/state.svelte';
  import { desktop, enterCompact, setCompactOnStart } from './lib/desktop.svelte';
  import Timer from './components/Timer.svelte';
  import CompactTimer from './components/CompactTimer.svelte';
  import SessionList from './components/SessionList.svelte';
  import ProjectsView from './components/ProjectsView.svelte';
  import ReportView from './components/ReportView.svelte';

  type Tab = 'sessions' | 'projects' | 'report';
  const tabs: { id: Tab; label: string }[] = [
    { id: 'sessions', label: 'Sessions' },
    { id: 'projects', label: 'Projects' },
    { id: 'report', label: 'Report' },
  ];

  let tab = $state<Tab>((location.hash.slice(1) as Tab) || 'sessions');

  // "v0.5.0" from a release tag; anything else (a local build) is just "dev".
  const version = $derived.by(() => {
    const v = app.view?.app_version;
    return v && v !== 'dev' ? `v${v}` : 'dev';
  });
  $effect(() => {
    if (!tabs.some((t) => t.id === tab)) tab = 'sessions';
    location.hash = tab;
  });

  // Collapse into the floating strip when a session starts. A session that was
  // already running when the app opened doesn't count.
  let lastActive: string | null | undefined;
  $effect(() => {
    if (!app.view) return;
    const active = app.view.active_session_id;
    if (lastActive === null && active && desktop.compactOnStart) enterCompact();
    lastActive = active;
  });

  onMount(() => {
    void refresh();
    return startClock();
  });
</script>

{#if desktop.compact}
  <CompactTimer />
{:else}
  <header class="top">
    <div class="brand">
      <span class="logo" aria-hidden="true"></span>
      <h1>Harmony</h1>
    </div>
    <nav>
      {#each tabs as t (t.id)}
        <button class:active={tab === t.id} class="ghost" onclick={() => (tab = t.id)}>{t.label}</button>
      {/each}
    </nav>
  </header>

  {#if desktop.available}
    <div class="float row muted">
      <label class="row">
        <input
          type="checkbox"
          checked={desktop.compactOnStart}
          onchange={(e) => setCompactOnStart(e.currentTarget.checked)}
        />
        Float compact while a session runs
      </label>
      <button class="ghost" onclick={enterCompact} title="Collapse into the floating compact window">⤡ Compact</button>
    </div>
  {/if}

  {#if app.error}
    <div class="error card" role="alert">
      <span class="grow">{app.error}</span>
      <button class="ghost" onclick={dismissError} aria-label="Dismiss">✕</button>
    </div>
  {/if}

  {#if !app.loaded}
    <p class="muted">Loading…</p>
  {:else}
    <Timer />
    {#if tab === 'sessions'}
      <SessionList />
    {:else if tab === 'projects'}
      <ProjectsView />
    {:else}
      <ReportView />
    {/if}
    <div class="version muted mono" title="Harmony version">
      {version}
    </div>
  {/if}
{/if}

<style>
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 16px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .logo {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 3px solid var(--accent);
  }
  h1 {
    font-size: 18px;
    letter-spacing: 0.02em;
  }
  nav {
    display: flex;
    gap: 2px;
  }
  nav button.active {
    color: var(--text);
    background: var(--surface-2);
  }
  .float {
    justify-content: flex-end;
    margin: -8px 0 10px;
    font-size: 13px;
  }
  .float label {
    gap: 6px;
    cursor: pointer;
  }
  .error {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    margin-bottom: 12px;
    border-color: var(--danger);
    color: var(--danger);
  }
  .version {
    position: fixed;
    right: 10px;
    bottom: 8px;
    font-size: 11px;
    opacity: 0.6;
    pointer-events: none;
    user-select: none;
  }
</style>
