import type { AgentTask } from '@/lib/agent-tasks';

const CHATS_API_BASE = '/api/chats';
const ORCHESTRATOR_TASKS_API_BASE = '/api/orchestrator-tasks';

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
