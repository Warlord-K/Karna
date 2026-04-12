// Types
export type AgentTaskStatus = 'todo' | 'planning' | 'plan_review' | 'in_progress' | 'review' | 'done' | 'failed' | 'cancelled';
export type AgentTaskPriority = 'low' | 'medium' | 'high' | 'urgent';

export interface AgentTask {
  id: string;
  user_id: string;
  task_number: number;
  title: string;
  description: string | null;
  repo: string | null;
  target_branch: string;
  status: AgentTaskStatus;
  priority: AgentTaskPriority;
  position: number;
  branch: string | null;
  pr_url: string | null;
  pr_number: number | null;
  plan_content: string | null;
  feedback: string | null;
  agent_session_id: string | null;
  error_message: string | null;
  cli: string | null;
  model: string | null;
  cost_usd: number;
  parent_task_id: string | null;
  created_at: string;
  updated_at: string;
  started_at: string | null;
  completed_at: string | null;
  // Populated client-side from fetched data
  subtasks?: AgentTask[];
  subtask_count?: number;
  subtask_done_count?: number;
}

export interface AgentLog {
  id: string;
  task_id: string;
  phase: string;
  message: string;
  log_type: 'info' | 'error' | 'command' | 'output' | 'claude' | 'tool' | 'comment';
  metadata: Record<string, unknown> | null;
  created_at: string;
}

// Column configuration
export const AGENT_COLUMNS = ['todo', 'plan', 'in_progress', 'review', 'done', 'failed'] as const;
export type AgentColumn = typeof AGENT_COLUMNS[number];

export const COLUMN_CONFIG: Record<AgentColumn, { label: string; color: string; statuses: AgentTaskStatus[] }> = {
  todo:        { label: 'Todo',        color: '#a09e97', statuses: ['todo'] },
  plan:        { label: 'Plan',        color: '#e5b847', statuses: ['planning', 'plan_review'] },
  in_progress: { label: 'In Progress', color: '#e5b847', statuses: ['in_progress'] },
  review:      { label: 'Review',      color: '#60a5a0', statuses: ['review'] },
  done:        { label: 'Done',        color: '#6ab070', statuses: ['done', 'cancelled'] },
  failed:      { label: 'Failed',      color: '#d4583a', statuses: ['failed'] },
};

export const PRIORITY_ORDER: Record<AgentTaskPriority, number> = {
  urgent: 0, high: 1, medium: 2, low: 3,
};

export const PRIORITY_COLORS: Record<AgentTaskPriority, string> = {
  urgent: '#d4583a',
  high: '#e08a3e',
  medium: '#e5b847',
  low: '#82807a',
};

export function getColumnForStatus(status: AgentTaskStatus): AgentColumn {
  for (const [col, config] of Object.entries(COLUMN_CONFIG)) {
    if (config.statuses.includes(status)) return col as AgentColumn;
  }
  return 'todo';
}

// API client helpers
const API_BASE = '/api/tasks';

export async function fetchTasks(): Promise<AgentTask[]> {
  const res = await fetch(API_BASE);
  if (!res.ok) throw new Error('Failed to fetch tasks');
  return res.json();
}

export async function createTask(data: {
  title: string;
  description: string;
  repo: string | null;
  priority: AgentTaskPriority;
  cli: string | null;
  model: string | null;
}): Promise<AgentTask> {
  const res = await fetch(API_BASE, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  });
  if (!res.ok) throw new Error('Failed to create task');
  return res.json();
}

export async function updateTask(id: string, updates: Partial<AgentTask>): Promise<void> {
  const res = await fetch(`${API_BASE}/${id}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(updates),
  });
  if (!res.ok) throw new Error('Failed to update task');
}

export async function deleteTask(id: string): Promise<void> {
  const res = await fetch(`${API_BASE}/${id}`, { method: 'DELETE' });
  if (!res.ok) throw new Error('Failed to delete task');
}

export async function fetchLogs(taskId: string): Promise<AgentLog[]> {
  const res = await fetch(`${API_BASE}/${taskId}/logs`);
  if (!res.ok) throw new Error('Failed to fetch logs');
  return res.json();
}

export async function postComment(taskId: string, message: string): Promise<AgentLog> {
  const res = await fetch(`${API_BASE}/${taskId}/comments`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ message }),
  });
  if (!res.ok) throw new Error('Failed to post comment');
  return res.json();
}

export async function fetchSubtasks(taskId: string): Promise<AgentTask[]> {
  const res = await fetch(`${API_BASE}/${taskId}/subtasks`);
  if (!res.ok) throw new Error('Failed to fetch subtasks');
  return res.json();
}

export async function approveWithSubtasks(taskId: string): Promise<AgentTask[]> {
  const res = await fetch(`${API_BASE}/${taskId}/subtasks`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
  });
  if (!res.ok) {
    const data = await res.json().catch(() => ({}));
    throw new Error(data.error || 'Failed to create subtasks');
  }
  return res.json();
}

/** Parse subtask definitions from plan_content markdown */
export function parseSubtasksFromPlan(planContent: string): { title: string; repo: string; description: string }[] {
  const match = planContent.match(/<!--\s*subtasks\s*\n([\s\S]*?)\nsubtasks\s*-->/);
  if (!match) return [];
  try {
    const parsed = JSON.parse(match[1]);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (s: any) => s && typeof s.title === 'string' && typeof s.repo === 'string'
    );
  } catch {
    return [];
  }
}

/** Check whether a task is a parent with subtasks defined in its plan */
export function hasSubtaskDefinitions(task: AgentTask): boolean {
  if (!task.plan_content) return false;
  return parseSubtasksFromPlan(task.plan_content).length > 0;
}

/** Nest subtasks under parent tasks. Returns top-level tasks only (with subtasks populated). */
export function nestSubtasks(tasks: AgentTask[]): AgentTask[] {
  const subtasksByParent = new Map<string, AgentTask[]>();
  const topLevel: AgentTask[] = [];

  for (const task of tasks) {
    if (task.parent_task_id) {
      const existing = subtasksByParent.get(task.parent_task_id) || [];
      existing.push(task);
      subtasksByParent.set(task.parent_task_id, existing);
    }
  }

  for (const task of tasks) {
    if (!task.parent_task_id) {
      const subs = subtasksByParent.get(task.id) || [];
      task.subtasks = subs;
      task.subtask_count = subs.length;
      task.subtask_done_count = subs.filter(s => s.status === 'done' || s.status === 'cancelled').length;
      if (subs.length > 0) {
        task.cost_usd += subs.reduce((sum, s) => sum + s.cost_usd, 0);
      }
      topLevel.push(task);
    }
  }

  return topLevel;
}

export function getTasksForColumn(tasks: AgentTask[], column: AgentColumn, includeSubtasks = false): AgentTask[] {
  const statuses = COLUMN_CONFIG[column].statuses;
  return tasks.filter(t => {
    if (!includeSubtasks && t.parent_task_id) return false;
    return statuses.includes(t.status);
  });
}
