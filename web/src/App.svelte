<script lang="ts">
  import { onMount } from 'svelte';
  import { app, refresh, startClock, dismissError } from './lib/state.svelte';
  import Timer from './components/Timer.svelte';
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
  $effect(() => {
    if (!tabs.some((t) => t.id === tab)) tab = 'sessions';
    location.hash = tab;
  });

  onMount(() => {
    void refresh();
    return startClock();
  });
</script>

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
  .error {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    margin-bottom: 12px;
    border-color: var(--danger);
    color: var(--danger);
  }
</style>
