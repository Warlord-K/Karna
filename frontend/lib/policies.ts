// Policy types + API client.
//
// Policies are advisory guardrails surfaced on the plan_review tab. When a
// task's plan touches paths matching `path_glob` in a matching repo, the UI
// shows a banner with the policy's `message`. Severity `block` is reserved
// for future enforcement; today it renders the same as `warn`.

export type PolicySeverity = 'warn' | 'block';

export interface Policy {
  id: string;
  name: string;
  /** "owner/repo" exact, "owner/*" prefix, or "*" all repos. */
  repo_pattern: string;
  /** Glob against file paths in plan_content. Supports `**` and `*`. */
  path_glob: string;
  message: string;
  severity: PolicySeverity;
  enabled: boolean;
  created_at: string | null;
  updated_at: string | null;
}

/** Stored on `agent_tasks.policy_matches` after the planner finishes. */
export interface PolicyMatch {
  policy_id: string;
  name: string;
  severity: PolicySeverity;
  message: string;
  paths: string[];
}

const API_BASE = '/api/policies';

export async function fetchPolicies(signal?: AbortSignal): Promise<Policy[]> {
  const res = await fetch(API_BASE, { signal });
  if (!res.ok) throw new Error('Failed to fetch policies');
  return res.json();
}

export async function createPolicy(data: {
  name: string;
  repo_pattern?: string;
  path_glob: string;
  message: string;
  severity?: PolicySeverity;
}): Promise<Policy> {
  const res = await fetch(API_BASE, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  });
  if (!res.ok) throw new Error('Failed to create policy');
  return res.json();
}

export async function updatePolicy(id: string, data: Partial<Policy>): Promise<void> {
  const res = await fetch(`${API_BASE}/${id}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  });
  if (!res.ok) throw new Error('Failed to update policy');
}

export async function deletePolicy(id: string): Promise<void> {
  const res = await fetch(`${API_BASE}/${id}`, { method: 'DELETE' });
  if (!res.ok) throw new Error('Failed to delete policy');
}
