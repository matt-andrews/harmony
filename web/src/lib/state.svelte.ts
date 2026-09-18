// Single reactive store. Every API mutation returns the full StateView, which
// replaces `app.view`, so the UI never has to patch local copies.

import * as api from './api';
import type { ProjectView, SessionView, StateView, UpdateProject, UpdateSession } from './api';

export type PickChoice =
  | { kind: 'project'; projectId: string; newTask: boolean }
  | { kind: 'create'; name: string; newTask: boolean }
  | { kind: 'untag' };

export const app = $state({
  view: null as StateView | null,
  loaded: false,
  busy: false,
  error: null as string | null,
  now: Date.now(),
});

export function projectById(id: string | null | undefined): ProjectView | undefined {
  if (!id) return undefined;
  return app.view?.projects.find((p) => p.id === id);
}

export function activeSession(): SessionView | undefined {
  const id = app.view?.active_session_id;
  return id ? app.view?.sessions.find((s) => s.id === id) : undefined;
}

/** Live elapsed seconds; ticks for the running session. */
export function elapsedSecs(s: SessionView): number {
  if (s.ended_at) return s.duration_secs;
  return Math.max(0, (app.now - Date.parse(s.started_at)) / 1000);
}

/** Seconds the running session has gained since the server computed the view. */
function liveDrift(taskId: string | null): number {
  const a = activeSession();
  return a && a.task_id === taskId ? elapsedSecs(a) - a.duration_secs : 0;
}

/** Total for the session's whole task; ticks while that task is running. */
export function taskTotalSecs(s: SessionView): number | null {
  if (s.task_total_secs === null) return null;
  return s.task_total_secs + liveDrift(s.task_id);
}

/** Live pay; recomputed client-side for the running session. */
export function livePay(s: SessionView): number {
  if (s.ended_at) return s.pay;
  const rate = projectById(s.project_id)?.hourly_rate ?? 0;
  return Math.round((elapsedSecs(s) / 3600) * rate * 100) / 100;
}

async function run<T>(fn: () => Promise<T>): Promise<T | undefined> {
  app.busy = true;
  app.error = null;
  try {
    return await fn();
  } catch (e) {
    app.error = e instanceof Error ? e.message : String(e);
    return undefined;
  } finally {
    app.busy = false;
  }
}

export function dismissError(): void {
  app.error = null;
}

export async function refresh(): Promise<void> {
  await run(async () => {
    app.view = await api.getState();
    app.loaded = true;
  });
}

/** Resolve a picker choice to a project id, creating the project if asked. */
async function ensureProject(choice: PickChoice): Promise<string | null> {
  switch (choice.kind) {
    case 'untag':
      return null;
    case 'project':
      return choice.projectId;
    case 'create': {
      const created = await api.createProject(choice.name);
      app.view = created.state;
      return created.project.id;
    }
  }
}

function wantsNewTask(choice: PickChoice): boolean {
  return choice.kind !== 'untag' && choice.newTask;
}

export async function startSession(choice: PickChoice | null): Promise<void> {
  await run(async () => {
    const projectId = choice ? await ensureProject(choice) : null;
    app.view = await api.startSession(projectId, choice ? wantsNewTask(choice) : false);
  });
}

export async function stopSession(): Promise<void> {
  await run(async () => {
    app.view = await api.stopSession();
  });
}

export async function tagSession(sessionId: string, choice: PickChoice): Promise<void> {
  await run(async () => {
    const projectId = await ensureProject(choice);
    app.view = await api.updateSession(sessionId, {
      project_id: projectId,
      new_task: wantsNewTask(choice),
    });
  });
}

export async function updateSession(sessionId: string, patch: UpdateSession): Promise<boolean> {
  const ok = await run(async () => {
    app.view = await api.updateSession(sessionId, patch);
    return true;
  });
  return ok === true;
}

export async function deleteSession(sessionId: string): Promise<void> {
  await run(async () => {
    app.view = await api.deleteSession(sessionId);
  });
}

export async function updateProject(projectId: string, patch: UpdateProject): Promise<boolean> {
  const ok = await run(async () => {
    app.view = await api.updateProject(projectId, patch);
    return true;
  });
  return ok === true;
}

/** Start the 1 s clock and a slow background refresh while a session runs. */
export function startClock(): () => void {
  const tick = setInterval(() => {
    app.now = Date.now();
  }, 1000);
  const slow = setInterval(() => {
    if (app.view?.active_session_id && !app.busy) void refresh();
  }, 60_000);
  return () => {
    clearInterval(tick);
    clearInterval(slow);
  };
}
