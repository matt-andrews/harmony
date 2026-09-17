<script lang="ts">
  import type { SessionView } from '../lib/api';
  import { app, elapsedSecs } from '../lib/state.svelte';
  import { dayKey, fmtDay, fmtDuration } from '../lib/time';
  import SessionCard from './SessionCard.svelte';

  interface Group {
    key: string;
    label: string;
    sessions: SessionView[];
    totalSecs: number;
  }

  // Sessions arrive newest first; group by local calendar day.
  const groups = $derived.by((): Group[] => {
    const out: Group[] = [];
    for (const s of app.view?.sessions ?? []) {
      const key = dayKey(s.started_at);
      let g = out[out.length - 1];
      if (!g || g.key !== key) {
        g = { key, label: fmtDay(s.started_at), sessions: [], totalSecs: 0 };
        out.push(g);
      }
      g.sessions.push(s);
      g.totalSecs += elapsedSecs(s);
    }
    return out;
  });
</script>

{#if groups.length === 0}
  <p class="muted empty">No sessions yet. Press Start.</p>
{/if}

{#each groups as g (g.key)}
  <section class="day">
    <h2 class="row">
      <span>{g.label}</span>
      <span class="grow"></span>
      <span class="muted mono total">{fmtDuration(g.totalSecs)}</span>
    </h2>
    {#each g.sessions as s (s.id)}
      <SessionCard session={s} />
    {/each}
  </section>
{/each}

<style>
  .day {
    margin-bottom: 20px;
  }
  h2 {
    font-size: 13px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
    margin: 0 0 8px 4px;
  }
  .total {
    font-size: 13px;
    text-transform: none;
    letter-spacing: 0;
  }
  .empty {
    text-align: center;
    padding: 40px 0;
  }
</style>
