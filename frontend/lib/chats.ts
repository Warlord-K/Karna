import type { AgentTask, AgentTaskStatus } from '@/lib/agent-tasks';
import type { BadgeTone } from '@/components/ui/badge';

const CHATS_API_BASE = '/api/chats';
const ORCHESTRATOR_TASKS_API_BASE = '/api/orchestrator-tasks';

/** Map a chat/task status to a human label + Badge tone, shared by the list and
 *  conversation views so status chips stay consistent across the chat surface. */
export function chatStatusBadge(status: AgentTaskStatus): { label: string; tone: BadgeTone } {
  switch (status) {
    case 'planning':
      return { label: 'Planning', tone: 'warning' };
    case 'plan_review':
      return { label: 'Plan ready', tone: 'warning' };
    case 'in_progress':
      return { label: 'Working', tone: 'warning' };
    case 'review':
      return { label: 'In review', tone: 'info' };
    case 'done':
      return { label: 'Done', tone: 'success' };
    case 'failed':
      return { label: 'Failed', tone: 'danger' };
    case 'cancelled':
      return { label: 'Cancelled', tone: 'neutral' };
    case 'todo':
    default:
      return { label: 'Queued', tone: 'neutral' };
  }
}

/** True while the agent is actively producing output (drives "working" affordances). */
export function isChatWorking(status: AgentTaskStatus): boolean {
  return status === 'planning' || status === 'in_progress';
}

export async function fetchChats(signal?: AbortSignal): Promise<AgentTask[]> {
  const res = await fetch(CHATS_API_BASE, { signal });
  if (!res.ok) throw new Error('Failed to fetch chats');
  return res.json();
}

export function summarizeChatTitle(message: string): string {
  const normalized = message.replace(/\s+/g, ' ').trim();
  if (!normalized) return 'New chat';
  const firstSentence = normalized.split(/[.!?](\s|$)/)[0]?.trim() || normalized;
  const capped = firstSentence.slice(0, 72).trim();
  return capped || normalized.slice(0, 72).trim() || 'New chat';
}

export async function createChat(data: {
  message: string;
  repo?: string | null;
  title?: string;
}): Promise<AgentTask> {
  const message = data.message.trim();
  if (!message) {
    throw new Error('Message is required');
  }

  const res = await fetch(ORCHESTRATOR_TASKS_API_BASE, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      title: data.title?.trim() || summarizeChatTitle(message),
      description: message,
      repo: data.repo || null,
      source: 'chat',
    }),
  });
  if (!res.ok) throw new Error('Failed to create chat');
  return res.json();
}
