export type RepoProfileStatus = 'pending' | 'onboarding' | 'ready' | 'failed' | 'stale';

export interface RepoProfile {
  id: string;
  user_id: string;
  repo: string;
  branch: string;
  status: RepoProfileStatus;
  summary: string | null;
  profile_json: Record<string, unknown> | null;
  last_onboarded_at: string | null;
  last_commit_sha: string | null;
  error_message: string | null;
  cost_usd: number;
  sync_issues: boolean;
  created_at: string;
  updated_at: string;
}

const API_BASE = '/api/repos';

export async function fetchRepos(signal?: AbortSignal): Promise<RepoProfile[]> {
  const res = await fetch(API_BASE, { signal });
  if (!res.ok) throw new Error('Failed to fetch repos');
  return res.json();
}

export async function addRepo(data: { repo: string; branch?: string }): Promise<RepoProfile> {
  const res = await fetch(API_BASE, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  });
  if (!res.ok) throw new Error('Failed to add repo');
  return res.json();
}

export async function deleteRepo(id: string): Promise<void> {
  const res = await fetch(`${API_BASE}/${id}`, { method: 'DELETE' });
  if (!res.ok) throw new Error('Failed to delete repo');
}

export async function triggerOnboard(id: string): Promise<void> {
  const res = await fetch(`${API_BASE}/${id}/onboard`, { method: 'POST' });
  if (!res.ok) throw new Error('Failed to trigger onboarding');
}

export async function updateRepo(id: string, data: { sync_issues?: boolean }): Promise<RepoProfile> {
  const res = await fetch(`${API_BASE}/${id}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  });
  if (!res.ok) throw new Error('Failed to update repo');
  return res.json();
}

export const REPO_STATUS_COLORS: Record<RepoProfileStatus, string> = {
  pending:    '#a09e97',
  onboarding: '#e5b847',
  ready:      '#6ab070',
  failed:     '#d4583a',
  stale:      '#e08a3e',
};

export const REPO_STATUS_LABELS: Record<RepoProfileStatus, string> = {
  pending:    'Pending',
  onboarding: 'Onboarding…',
  ready:      'Ready',
  failed:     'Failed',
  stale:      'Stale',
};
