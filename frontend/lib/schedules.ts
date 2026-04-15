export type SchedulePriority = 'low' | 'medium' | 'high' | 'urgent';
export type ScheduledRunStatus = 'running' | 'completed' | 'failed';

export interface Schedule {
  id: string;
  user_id: string;
  name: string;
  prompt: string;
  repos: string | null;
  cron_expression: string | null;
  run_at: string | null;
  skills: string[] | null;
  mcp_servers: string[] | null;
  max_open_tasks: number;
  task_prefix: string | null;
  priority: SchedulePriority;
  cli: string | null;
  model: string | null;
  enabled: boolean;
  created_at: string;
  updated_at: string;
  // Joined from last run
  last_run?: ScheduledRun | null;
}

export interface ScheduledRun {
  id: string;
  schedule_id: string;
  status: ScheduledRunStatus;
  started_at: string;
  completed_at: string | null;
  summary_markdown: string | null;
  tasks_created: string[];
  task_count: number;
  cost_usd: number;
  created_at: string;
}

export interface ScheduledRunLog {
  id: string;
  run_id: string;
  level: 'info' | 'error' | 'warn' | 'debug';
  message: string;
  created_at: string;
}

// API client helpers
const API_BASE = '/api/schedules';

export async function fetchSchedules(signal?: AbortSignal): Promise<Schedule[]> {
  const res = await fetch(API_BASE, { signal });
  if (!res.ok) throw new Error('Failed to fetch schedules');
  return res.json();
}

export async function createSchedule(data: {
  name: string;
  prompt: string;
  repos?: string | null;
  cron_expression?: string | null;
  run_at?: string | null;
  skills?: string[];
  mcp_servers?: string[];
  max_open_tasks?: number;
  task_prefix?: string | null;
  priority?: SchedulePriority;
  cli?: string | null;
  model?: string | null;
}): Promise<Schedule> {
  const res = await fetch(API_BASE, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  });
  if (!res.ok) throw new Error('Failed to create schedule');
  return res.json();
}

export async function updateSchedule(id: string, updates: Partial<Schedule>): Promise<void> {
  const res = await fetch(`${API_BASE}/${id}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(updates),
  });
  if (!res.ok) throw new Error('Failed to update schedule');
}

export async function deleteSchedule(id: string): Promise<void> {
  const res = await fetch(`${API_BASE}/${id}`, { method: 'DELETE' });
  if (!res.ok) throw new Error('Failed to delete schedule');
}

export async function triggerSchedule(id: string): Promise<void> {
  const res = await fetch(`${API_BASE}/${id}/trigger`, { method: 'POST' });
  if (!res.ok) throw new Error('Failed to trigger schedule');
}

export async function fetchRuns(scheduleId: string, signal?: AbortSignal): Promise<ScheduledRun[]> {
  const res = await fetch(`${API_BASE}/${scheduleId}/runs`, { signal });
  if (!res.ok) throw new Error('Failed to fetch runs');
  return res.json();
}

export async function fetchRunLogs(scheduleId: string, runId: string, signal?: AbortSignal): Promise<ScheduledRunLog[]> {
  const res = await fetch(`${API_BASE}/${scheduleId}/runs/${runId}/logs`, { signal });
  if (!res.ok) throw new Error('Failed to fetch run logs');
  return res.json();
}

// Cron expression helpers for display
const CRON_PRESETS: Record<string, string> = {
  '* * * * *':     'Every minute',
  '*/5 * * * *':   'Every 5 minutes',
  '*/15 * * * *':  'Every 15 minutes',
  '*/30 * * * *':  'Every 30 minutes',
  '0 * * * *':     'Every hour',
  '0 */2 * * *':   'Every 2 hours',
  '0 */4 * * *':   'Every 4 hours',
  '0 */6 * * *':   'Every 6 hours',
  '0 */12 * * *':  'Every 12 hours',
  '0 0 * * *':     'Daily at midnight',
  '0 9 * * *':     'Daily at 9am',
  '0 9 * * 1':     'Every Monday at 9am',
  '0 9 * * 1-5':   'Weekdays at 9am',
};

export function humanizeCron(cron: string): string {
  return CRON_PRESETS[cron] || cron;
}

export const RUN_STATUS_COLORS: Record<ScheduledRunStatus, string> = {
  running:   '#e5b847',
  completed: '#6ab070',
  failed:    '#d4583a',
};
