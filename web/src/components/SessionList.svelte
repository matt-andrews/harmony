<script lang="ts">
  import type { SessionView } from '../lib/api';
  import { app, elapsedSecs } from '../lib/state.svelte';
  import { dayKey, fmtDay, fmtDuration } from '../lib/time';
  import SessionCard from './SessionCard.svelte';

  /** A run of consecutive sessions on one task (or consecutive untagged ones). */
  interface Cluster {
    key: string;
    taskId: string | null;
    sessions: SessionView[];
  }

  interface Group {
    key: string;
    label: string;
    clusters: Cluster[];
    totalSecs: number;
  }

  // Sessions arrive newest first; group by local calendar day, then cluster
  // by task within the day so the eye can tell one pickup from the next.
  const groups = $derived.by((): Group[] => {
    const out: Group[] = [];
    for (const s of app.view?.sessions ?? []) {
      const key = dayKey(s.started_at);
      let g = out[out.length - 1];
      if (!g || g.key !== key) {
        g = { key, label: fmtDay(s.started_at), clusters: [], totalSecs: 0 };
        out.push(g);
      }
      let c = g.clusters[g.clusters.length - 1];
      if (!c || c.taskId !== s.task_id) {
        // Keyed by the first session: a task can recur non-adjacently in a day.
        c = { key: s.id, taskId: s.task_id, sessions: [] };
        g.clusters.push(c);
      }
      c.sessions.push(s);
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
    {#each g.clusters as c (c.key)}
      <div class="cluster">
        {#each c.sessions as s (s.id)}
          <SessionCard session={s} />
        {/each}
      </div>
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
  .cluster {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  /* The barrier between one task and the next within a day. */
  .cluster + .cluster {
    margin-top: 14px;
    padding-top: 14px;
    border-top: 1px solid var(--border-hover);
  }
  .empty {
    text-align: center;
    padding: 40px 0;
  }
</style>
