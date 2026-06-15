'use client';

import { useState } from 'react';
import { useParams, useRouter } from 'next/navigation';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import toast from 'react-hot-toast';
import { formatDistanceToNow } from 'date-fns';
import {
  RepoProfile,
  REPO_STATUS_COLORS,
  REPO_STATUS_LABELS,
  PrReview,
  fetchRepoReviews,
  triggerWebhookRegister,
} from '@/lib/repos';
import { useRepos, useDeleteRepo, useTriggerOnboard, useUpdateRepo } from '@/hooks/use-repos';
import { useAgents } from '@/hooks/use-tasks';
import {
  ArrowLeft,
  ArrowsClockwise,
  Trash,
  GitBranch,
  ArrowSquareOut,
} from '@phosphor-icons/react';
import { MarkdownContent } from '@/components/agent/markdown-content';
import { ReviewLogModal } from '@/components/agent/review-log-modal';

export default function RepoDetailPage() {
  const params = useParams();
  const router = useRouter();
  const qc = useQueryClient();
  const id = params.id as string;

  const { data: repos = [] } = useRepos();
  const repo = repos.find(r => r.id === id) ?? null;

  const deleteMutation = useDeleteRepo();
  const onboardMutation = useTriggerOnboard();
  const updateMutation = useUpdateRepo();
  const { data: agents = [] } = useAgents(!!repo);

  const [openReview, setOpenReview] = useState<PrReview | null>(null);

  const { data: reviews = [] } = useQuery<PrReview[]>({
    queryKey: ['repos', repo?.id, 'reviews'],
    queryFn: ({ signal }) => fetchRepoReviews(repo!.id, signal),
    enabled: !!repo && !!repo.review_prs,
    refetchInterval: (q) => {
      const data = q.state.data as PrReview[] | undefined;
      const hasRunning = data?.some((r) => r.status === 'running' || r.status === 'pending');
      return hasRunning ? 3000 : 15000;
    },
  });

  const rereginMutation = useMutation({
    mutationFn: () => triggerWebhookRegister(repo!.id),
    onSuccess: () => {
      toast.success('Webhook re-registration queued');
      qc.invalidateQueries({ queryKey: ['repos'] });
    },
    onError: () => toast.error('Failed to queue re-registration'),
  });

  if (!repo) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="w-5 h-5 border-2 border-gray-7 border-t-gray-12 rounded-full animate-spin" />
      </div>
    );
  }

  const handleOnboard = async () => {
    await onboardMutation.mutateAsync(repo.id);
  };

  const handleDelete = async () => {
    await deleteMutation.mutateAsync(repo.id);
    router.push('/repos');
  };

  const statusColor = REPO_STATUS_COLORS[repo.status];
  const statusLabel = REPO_STATUS_LABELS[repo.status];
  // profile_json is an untyped JSON blob from the DB; treat it as a loose record
  // so field reads/renders don't surface as `unknown` in JSX children.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const profile = (repo.profile_json ?? {}) as Record<string, any>;
  const showWebhookRow = repo.sync_issues || repo.review_prs;

  return (
    <div className="h-full overflow-y-auto">
      <div className="w-full px-4 sm:px-6 lg:px-8 py-6">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div className="flex items-center gap-3 min-w-0">
            <button
              onClick={() => router.push('/repos')}
              className="h-8 w-8 flex items-center justify-center text-gray-8 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors flex-shrink-0"
            >
              <ArrowLeft size={16} weight="bold" />
            </button>
            <span
              className="w-2.5 h-2.5 rounded-full flex-shrink-0"
              style={{ backgroundColor: statusColor }}
            />
            <h1 className="text-[18px] font-semibold text-gray-12 truncate font-mono">{repo.repo}</h1>
            <span
              className="px-2 py-0.5 rounded text-[11px] font-medium flex-shrink-0"
              style={{ backgroundColor: statusColor + '20', color: statusColor }}
            >
              {statusLabel}
            </span>
          </div>
          <div className="flex items-center gap-1 flex-shrink-0">
            <button
              onClick={handleOnboard}
              disabled={repo.status === 'onboarding'}
              title="Re-onboard"
              className="h-8 w-8 flex items-center justify-center text-gray-8 hover:text-sun-9 hover:bg-gray-3 rounded-lg transition-colors disabled:opacity-40"
            >
              <ArrowsClockwise size={16} weight="bold" className={repo.status === 'onboarding' ? 'animate-spin' : ''} />
            </button>
            <button
              onClick={handleDelete}
              title="Delete"
              className="h-8 w-8 flex items-center justify-center text-gray-8 hover:text-red-400 hover:bg-gray-3 rounded-lg transition-colors"
            >
              <Trash size={16} weight="bold" />
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="space-y-5">
          {/* Info grid */}
          <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
            <InfoItem label="Branch" value={repo.branch} icon={<GitBranch size={12} weight="bold" />} />
            <InfoItem label="Language" value={(profile.language as string) || '—'} />
            <InfoItem label="Framework" value={(profile.framework as string) === 'null' ? '—' : (profile.framework as string) || '—'} />
            <InfoItem label="Package Manager" value={(profile.package_manager as string) || '—'} />
            <InfoItem label="Cost" value={`$${repo.cost_usd.toFixed(4)}`} />
            {repo.last_commit_sha && (
              <InfoItem label="SHA" value={repo.last_commit_sha.slice(0, 8)} mono />
            )}
          </div>

          {/* Settings */}
          <div className="space-y-2">
            <h3 className="text-[13px] font-medium text-gray-10">Settings</h3>

            <div className="bg-gray-2 rounded-lg border border-gray-3 px-3 py-2.5 flex items-center justify-between">
              <div>
                <div className="text-[13px] text-gray-12">Sync GitHub Issues</div>
                <div className="text-[11px] text-gray-7 mt-0.5">Automatically create tasks from new GitHub issues</div>
              </div>
              <button
                onClick={() => updateMutation.mutate({ id: repo.id, data: { sync_issues: !repo.sync_issues } })}
                className={`relative w-9 h-5 rounded-full transition-colors flex-shrink-0 ${
                  repo.sync_issues ? 'bg-sun-9' : 'bg-gray-5'
                }`}
              >
                <span
                  className={`absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full transition-transform ${
                    repo.sync_issues ? 'translate-x-4' : 'translate-x-0'
                  }`}
                />
              </button>
            </div>

            <div className="bg-gray-2 rounded-lg border border-gray-3 px-3 py-2.5 flex items-center justify-between">
              <div>
                <div className="text-[13px] text-gray-12">Auto-review PRs</div>
                <div className="text-[11px] text-gray-7 mt-0.5">
                  When a teammate opens a PR, the agent posts a single review comment.
                  Uses your existing CLI subscription - no extra cost.
                </div>
              </div>
              <button
                onClick={() => updateMutation.mutate({ id: repo.id, data: { review_prs: !repo.review_prs } })}
                className={`relative w-9 h-5 rounded-full transition-colors flex-shrink-0 ${
                  repo.review_prs ? 'bg-sun-9' : 'bg-gray-5'
                }`}
              >
                <span
                  className={`absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full transition-transform ${
                    repo.review_prs ? 'translate-x-4' : 'translate-x-0'
                  }`}
                />
              </button>
            </div>

            {repo.review_prs && (
              <div className="bg-gray-2 rounded-lg border border-gray-3 px-3 py-2.5 flex items-center gap-3">
                <div className="flex-1 min-w-0">
                  <div className="text-[13px] text-gray-12">Review agent</div>
                  <div className="text-[11px] text-gray-7 mt-0.5">
                    Which agent profile reviews PRs for this repo. Pick a cheap model to keep quota burn low.
                  </div>
                </div>
                <select
                  value={repo.review_agent_id ?? ''}
                  onChange={(e) =>
                    updateMutation.mutate({
                      id: repo.id,
                      data: { review_agent_id: e.target.value === '' ? null : e.target.value },
                    })
                  }
                  className="h-8 px-2 rounded-md text-[12px] border bg-gray-3 border-gray-5 text-gray-12 cursor-pointer focus:outline-none flex-shrink-0"
                >
                  <option value="">Default agent</option>
                  {agents.map((a) => (
                    <option key={a.id} value={a.id} disabled={!!a.paused_reason}>
                      {a.avatar_emoji} {a.name}{a.paused_reason ? ' (paused)' : ''}
                    </option>
                  ))}
                </select>
              </div>
            )}

            {showWebhookRow && (
              <WebhookStatusRow
                repo={repo}
                onReregister={() => rereginMutation.mutate()}
                isReregistering={rereginMutation.isPending}
              />
            )}
          </div>

          {/* Commands */}
          {(profile.test_command || profile.lint_command || profile.build_command) && (
            <div className="space-y-2">
              <h3 className="text-[13px] font-medium text-gray-10">Commands</h3>
              <div className="bg-gray-2 rounded-lg border border-gray-3 p-3 space-y-1.5 font-mono text-[12px]">
                {profile.test_command && <CommandRow label="test" value={profile.test_command as string} />}
                {profile.lint_command && <CommandRow label="lint" value={profile.lint_command as string} />}
                {profile.build_command && <CommandRow label="build" value={profile.build_command as string} />}
              </div>
            </div>
          )}

          {/* Key directories */}
          {profile.key_directories && Object.keys(profile.key_directories as object).length > 0 && (
            <div className="space-y-2">
              <h3 className="text-[13px] font-medium text-gray-10">Key Directories</h3>
              <div className="bg-gray-2 rounded-lg border border-gray-3 p-3 space-y-1 font-mono text-[12px]">
                {Object.entries(profile.key_directories as Record<string, string>).map(([dir, desc]) => (
                  <div key={dir} className="flex gap-2">
                    <span className="text-sun-9 flex-shrink-0">{dir}</span>
                    <span className="text-gray-8">{desc}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Summary */}
          {repo.summary && (
            <div className="space-y-2">
              <h3 className="text-[13px] font-medium text-gray-10">Summary</h3>
              <div className="prose prose-invert prose-sm max-w-3xl text-[13px] text-gray-9">
                <MarkdownContent content={repo.summary} />
              </div>
            </div>
          )}

          {/* PR Reviews */}
          {repo.review_prs && (
            <div className="space-y-2">
              <h3 className="text-[13px] font-medium text-gray-10">PR reviews</h3>
              {reviews.length === 0 ? (
                <div className="bg-gray-2 rounded-lg border border-gray-3 px-3 py-3 text-[12px] text-gray-7">
                  No reviews yet. Will appear here as soon as a teammate opens a PR.
                </div>
              ) : (
                <div className="space-y-1.5">
                  {reviews.slice(0, 10).map((r) => (
                    <ReviewRow key={r.id} review={r} onClick={() => setOpenReview(r)} />
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Error */}
          {repo.status === 'failed' && repo.error_message && (
            <div className="space-y-2">
              <h3 className="text-[13px] font-medium text-red-400">Error</h3>
              <div className="bg-red-400/10 rounded-lg border border-red-400/20 p-3 text-[13px] text-red-300 font-mono whitespace-pre-wrap">
                {repo.error_message}
              </div>
            </div>
          )}
        </div>
      </div>

      {openReview && (
        <ReviewLogModal
          repoId={repo.id}
          review={openReview}
          onClose={() => setOpenReview(null)}
        />
      )}
    </div>
  );
}

function ReviewRow({ review, onClick }: { review: PrReview; onClick: () => void }) {
  const statusTone: Record<string, string> = {
    running: 'bg-amber-500/15 text-amber-400 border-amber-500/30 animate-pulse',
    pending: 'bg-amber-500/15 text-amber-400 border-amber-500/30',
    completed: 'bg-green-500/15 text-green-400 border-green-500/30',
    failed: 'bg-red-500/15 text-red-400 border-red-500/30',
    skipped: 'bg-gray-3 text-gray-8 border-gray-5',
  };
  const tone = statusTone[review.status] || statusTone.pending;
  return (
    <button
      onClick={onClick}
      className="w-full text-left bg-gray-2 border border-gray-3 rounded-lg px-3 py-2 hover:bg-gray-3 transition-colors"
    >
      <div className="flex items-center gap-2">
        <span className="text-[12px] text-gray-9 font-mono">#{review.pr_number}</span>
        <span className={`inline-flex items-center px-1.5 h-4 rounded text-[10px] border ${tone}`}>
          {review.status}
        </span>
        {review.author && <span className="text-[11px] text-gray-7">@{review.author}</span>}
        <span className="text-[11px] text-gray-7 font-mono ml-2">{review.head_sha.slice(0, 7)}</span>
        {review.cost_usd > 0 && (
          <span className="text-[11px] text-gray-7 font-mono">${review.cost_usd.toFixed(3)}</span>
        )}
        <span className="text-[11px] text-gray-7 ml-auto">
          {review.created_at ? formatDistanceToNow(new Date(review.created_at), { addSuffix: true }) : ''}
        </span>
        {review.pr_url && (
          <a
            href={review.pr_url}
            target="_blank"
            rel="noopener noreferrer"
            onClick={(e) => e.stopPropagation()}
            className="text-gray-7 hover:text-gray-11 flex-shrink-0"
            title="Open PR on GitHub"
          >
            <ArrowSquareOut size={12} weight="bold" />
          </a>
        )}
      </div>
      {review.error_message && (
        <div className="text-[11px] text-red-300 mt-1 line-clamp-2 font-mono">{review.error_message}</div>
      )}
    </button>
  );
}

function InfoItem({ label, value, icon, mono }: { label: string; value: string; icon?: React.ReactNode; mono?: boolean }) {
  return (
    <div className="bg-gray-2 rounded-lg border border-gray-3 px-3 py-2">
      <span className="text-[11px] text-gray-7 uppercase tracking-wider">{label}</span>
      <div className={`flex items-center gap-1.5 mt-0.5 text-[13px] text-gray-12 ${mono ? 'font-mono' : ''}`}>
        {icon}
        <span className="truncate">{value}</span>
      </div>
    </div>
  );
}

function CommandRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex gap-2">
      <span className="text-gray-7 w-10 flex-shrink-0 text-right">{label}</span>
      <span className="text-gray-11">{value}</span>
    </div>
  );
}

function WebhookStatusRow({
  repo,
  onReregister,
  isReregistering,
}: {
  repo: RepoProfile;
  onReregister: () => void;
  isReregistering: boolean;
}) {
  const status = repo.webhook_status;
  const toneByStatus: Record<string, { dot: string; text: string; label: string; hint: string }> = {
    registered: {
      dot: 'bg-green-500',
      text: 'text-green-400',
      label: 'Webhook active',
      hint: repo.webhook_url ? `Posted to ${repo.webhook_url}` : 'GitHub events will create tasks here',
    },
    not_registered: {
      dot: 'bg-amber-400',
      text: 'text-amber-400',
      label: 'Webhook pending',
      hint: 'Will register on the next agent poll cycle',
    },
    failed: {
      dot: 'bg-red-400',
      text: 'text-red-400',
      label: 'Webhook failed',
      hint: repo.webhook_error || 'Check that GITHUB_TOKEN has admin:repo_hook scope',
    },
    unsupported: {
      dot: 'bg-amber-400',
      text: 'text-amber-400',
      label: 'No public URL configured',
      hint: 'Set AGENT_WEBHOOK_URL or TUNNEL_AGENT_HOSTNAME on the agent to enable webhooks',
    },
  };
  const tone = toneByStatus[status] || toneByStatus.not_registered;

  return (
    <div className="bg-gray-2 rounded-lg border border-gray-3 px-3 py-2.5">
      <div className="flex items-center gap-2">
        <span className={`w-2 h-2 rounded-full flex-shrink-0 ${tone.dot}`} />
        <span className={`text-[13px] ${tone.text}`}>{tone.label}</span>
        {status !== 'unsupported' && (
          <button
            onClick={onReregister}
            disabled={isReregistering}
            className="ml-auto h-6 px-2 text-[11px] text-gray-9 hover:text-gray-12 hover:bg-gray-3 rounded transition-colors flex items-center gap-1 disabled:opacity-50"
            title="Force the agent to retry webhook registration on its next poll"
          >
            <ArrowsClockwise size={11} weight="bold" className={isReregistering ? 'animate-spin' : ''} />
            Re-register
          </button>
        )}
      </div>
      <div className="text-[11px] text-gray-7 mt-0.5 pl-4">{tone.hint}</div>
    </div>
  );
}
