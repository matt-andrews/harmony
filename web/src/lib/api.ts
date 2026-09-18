// Typed client for the Harmony HTTP API. Mirrors src/domain/views.rs.

export interface ProjectView {
  id: string;
  name: string;
  hourly_rate: number;
  color: string;
  created_at: string;
  archived: boolean;
  total_secs: number;
  total_pay: number;
  task_count: number;
  current_task_number: number | null;
  current_task_total_secs: number | null;
}

export interface SessionView {
  id: string;
  started_at: string;
  ended_at: string | null;
  note: string | null;
  duration_secs: number;
  pay: number;
  project_id: string | null;
  project_name: string | null;
  color: string | null;
  task_id: string | null;
  task_number: number | null;
  ordinal: number | null;
  task_session_count: number | null;
  task_total_secs: number | null;
}

export interface StateView {
  server_time: string;
  active_session_id: string | null;
  resume_project_id: string | null;
  projects: ProjectView[];
  sessions: SessionView[];
}

export interface TaskSummary {
  id: string;
  number: number;
  created_at: string;
  session_count: number;
  total_secs: number;
  total_pay: number;
  first_started: string | null;
  last_ended: string | null;
}

export interface ProjectSummary extends ProjectView {
  tasks: TaskSummary[];
}

export interface ReportRow {
  project_id: string | null;
  project_name: string;
  color: string | null;
  session_count: number;
  total_secs: number;
  total_pay: number;
}

export interface Report {
  from: string;
  to: string;
  rows: ReportRow[];
  total_secs: number;
  total_pay: number;
}

export interface UpdateProject {
  name?: string;
  hourly_rate?: number;
  color?: string;
  archived?: boolean;
}

export interface UpdateSession {
  project_id?: string | null;
  new_task?: boolean;
  task_id?: string;
  started_at?: string;
  ended_at?: string | null;
  note?: string | null;
}

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(path, {
    method,
    headers: body !== undefined ? { 'content-type': 'application/json' } : undefined,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  if (!res.ok) {
    let message = `${res.status} ${res.statusText}`;
    try {
      const data = await res.json();
      if (typeof data?.error === 'string') message = data.error;
    } catch {
      /* not json */
    }
    throw new ApiError(res.status, message);
  }
  return (await res.json()) as T;
}

export const getState = () => request<StateView>('GET', '/api/state');

export const createProject = (name: string, hourly_rate?: number) =>
  request<{ project: ProjectView; state: StateView }>('POST', '/api/projects', {
    name,
    hourly_rate,
  });

export const updateProject = (id: string, patch: UpdateProject) =>
  request<StateView>('PATCH', `/api/projects/${id}`, patch);

export const projectSummary = (id: string) =>
  request<ProjectSummary>('GET', `/api/projects/${id}/summary`);

export const startSession = (project_id: string | null, new_task: boolean) =>
  request<StateView>('POST', '/api/sessions/start', { project_id, new_task });

export const stopSession = () => request<StateView>('POST', '/api/sessions/stop');

export const updateSession = (id: string, patch: UpdateSession) =>
  request<StateView>('PATCH', `/api/sessions/${id}`, patch);

export const deleteSession = (id: string) => request<StateView>('DELETE', `/api/sessions/${id}`);

export const getReport = (from: Date, to: Date) =>
  request<Report>(
    'GET',
    `/api/report?from=${encodeURIComponent(from.toISOString())}&to=${encodeURIComponent(to.toISOString())}`,
  );
