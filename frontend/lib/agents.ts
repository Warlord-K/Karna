// Agent profile types + API client.
//
// Agent profiles are pseudo-users: named identities (Sonnet, Codex GPT-5.4)
// that show up alongside humans in the assignee picker. NULL assignment
// (neither human nor specific agent) means "any agent picks it up."

export interface AgentProfile {
  id: string;
  slug: string;
  name: string;
  avatar_emoji: string;
  cli: string;
  model: string;
  system_prompt_addendum: string | null;
  /** NULL = active. Set = paused with a human-readable reason. */
  paused_reason: string | null;
  is_default: boolean;
  created_at: string | null;
  updated_at: string | null;
}

export type Assignable =
  | { type: 'user'; id: string; name: string | null; email: string | null }
  | {
      type: 'agent';
      id: string;
      name: string;
      slug: string;
      avatar_emoji: string;
      paused: boolean;
    };

export async function fetchAgents(signal?: AbortSignal): Promise<AgentProfile[]> {
  const res = await fetch('/api/agents', { signal });
  if (!res.ok) throw new Error('Failed to fetch agents');
  return res.json();
}

export async function fetchAssignables(signal?: AbortSignal): Promise<Assignable[]> {
  const res = await fetch('/api/assignables', { signal });
  if (!res.ok) throw new Error('Failed to fetch assignables');
  return res.json();
}

export async function createAgent(data: {
  slug: string;
  name: string;
  cli: string;
  model: string;
  avatar_emoji?: string;
  system_prompt_addendum?: string;
}): Promise<AgentProfile> {
  const res = await fetch('/api/agents', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  });
  if (!res.ok) {
    if (res.status === 409) throw new Error('An agent with that slug already exists');
    throw new Error('Failed to create agent');
  }
  return res.json();
}

export async function updateAgent(id: string, updates: Partial<AgentProfile>): Promise<void> {
  const res = await fetch(`/api/agents/${id}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(updates),
  });
  if (!res.ok) throw new Error('Failed to update agent');
}

export async function deleteAgent(id: string): Promise<void> {
  const res = await fetch(`/api/agents/${id}`, { method: 'DELETE' });
  if (!res.ok) throw new Error('Failed to delete agent');
}
