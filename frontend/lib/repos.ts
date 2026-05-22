export type RepoProfileStatus = 'pending' | 'onboarding' | 'ready' | 'failed' | 'stale';
export type WebhookStatus = 'not_registered' | 'registered' | 'failed' | 'unsupported';

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
  /** When TRUE, the agent auto-reviews human-opened PRs on this repo. */
  review_prs: boolean;
  /** Which agent profile reviews PRs for this repo. NULL = config default. */
  review_agent_id: string | null;
  webhook_status: WebhookStatus;
  webhook_error: string | null;
  webhook_url: string | null;
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

export async function triggerWebhookRegister(id: string): Promise<void> {
  const res = await fetch(`${API_BASE}/${id}/webhook`, { method: 'POST' });
  if (!res.ok) throw new Error('Failed to re-register webhook');
}

// --- PR review types + clients ---

export type PrReviewStatus = 'pending' | 'running' | 'completed' | 'failed' | 'skipped';

export interface PrReview {
  id: string;
  repo: string;
  pr_number: number;
  pr_url: string | null;
  head_sha: string;
  author: string | null;
  reviewer_agent_id: string | null;
  status: PrReviewStatus;
  comments_posted: number;
  cost_usd: number;
  error_message: string | null;
  created_at: string | null;
  completed_at: string | null;
}

export interface PrReviewLog {
  id: string;
  review_id: string;
  phase: string;
  message: string;
  log_type: 'info' | 'error' | 'warning' | 'command' | 'output' | 'tool' | null;
  metadata: Record<string, unknown> | null;
  created_at: string | null;
}

export interface PrReviewFinding {
  id: string;
  review_id: string;
  path: string;
  line: number;
  start_line: number | null;
  side: 'LEFT' | 'RIGHT';
  body: string;
  posted: boolean;
  skip_reason: string | null;
  created_at: string | null;
}

export async function fetchRepoReviews(repoId: string, signal?: AbortSignal): Promise<PrReview[]> {
  const res = await fetch(`${API_BASE}/${repoId}/reviews`, { signal });
  if (!res.ok) throw new Error('Failed to fetch reviews');
  return res.json();
}

export async function fetchReviewLogs(repoId: string, reviewId: string, signal?: AbortSignal): Promise<PrReviewLog[]> {
  const res = await fetch(`${API_BASE}/${repoId}/reviews/${reviewId}/logs`, { signal });
  if (!res.ok) throw new Error('Failed to fetch review logs');
  return res.json();
}

export async function fetchReviewFindings(repoId: string, reviewId: string, signal?: AbortSignal): Promise<PrReviewFinding[]> {
  const res = await fetch(`${API_BASE}/${repoId}/reviews/${reviewId}/findings`, { signal });
  if (!res.ok) throw new Error('Failed to fetch review findings');
  return res.json();
}

export async function updateRepo(
  id: string,
  data: {
    sync_issues?: boolean;
    review_prs?: boolean;
    /** Pass undefined to leave unchanged, `null` to clear, a UUID to set. */
    review_agent_id?: string | null;
  },
): Promise<RepoProfile> {
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
