'use client';

import { RepoProfile, REPO_STATUS_COLORS, REPO_STATUS_LABELS } from '@/lib/repos';
import { ArrowsClockwise, GitBranch, Trash } from '@phosphor-icons/react';
import { formatDistanceToNow } from 'date-fns';

interface RepoCardProps {
  repo: RepoProfile;
  onClick: () => void;
  onOnboard: () => void;
  onDelete: () => void;
}

export function RepoCard({ repo, onClick, onOnboard, onDelete }: RepoCardProps) {
  const statusColor = REPO_STATUS_COLORS[repo.status];
  const statusLabel = REPO_STATUS_LABELS[repo.status];
  const language = repo.profile_json?.language as string | undefined;
  const framework = repo.profile_json?.framework as string | undefined;

  return (
    <div
      onClick={onClick}
      className="group bg-gray-2 border border-gray-3 rounded-lg p-4 hover:border-gray-5 transition-colors cursor-pointer"
    >
      {/* Top row: repo name + actions */}
      <div className="flex items-start justify-between gap-3 mb-2.5">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 mb-1">
            <span
              className="w-2 h-2 rounded-full flex-shrink-0"
              style={{ backgroundColor: statusColor }}
            />
            <h3 className="text-[14px] font-medium text-gray-12 truncate font-mono">
              {repo.repo}
            </h3>
          </div>
          {repo.summary && (
            <p className="text-[13px] text-gray-8 line-clamp-2">
              {repo.summary.replace(/^## Summary\s*\n?/, '').slice(0, 200)}
            </p>
          )}
        </div>

        <div className="flex items-center gap-1 flex-shrink-0" onClick={(e) => e.stopPropagation()}>
          <button
            onClick={onOnboard}
            disabled={repo.status === 'onboarding'}
            title="Re-onboard"
            className="h-7 w-7 flex items-center justify-center text-gray-8 hover:text-sun-9 hover:bg-gray-3 rounded-md transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <ArrowsClockwise
              size={14}
              weight="bold"
              className={repo.status === 'onboarding' ? 'animate-spin' : ''}
            />
          </button>
          <button
            onClick={onDelete}
            title="Remove"
            className="h-7 w-7 flex items-center justify-center text-gray-8 hover:text-red-400 hover:bg-gray-3 rounded-md transition-colors"
          >
            <Trash size={14} weight="bold" />
          </button>
        </div>
      </div>

      {/* Info row */}
      <div className="flex items-center gap-3 flex-wrap text-[12px] text-gray-8">
        <span className="flex items-center gap-1">
          <GitBranch size={12} weight="bold" />
          {repo.branch}
        </span>

        <span
          className="px-1.5 py-0.5 rounded text-[11px] font-medium"
          style={{ backgroundColor: statusColor + '20', color: statusColor }}
        >
          {statusLabel}
        </span>

        {language && (
          <span className="px-1.5 py-0.5 rounded bg-gray-3 text-gray-9 font-mono text-[11px]">
            {language}
          </span>
        )}

        {framework && framework !== 'null' && (
          <span className="px-1.5 py-0.5 rounded bg-gray-3 text-gray-9 font-mono text-[11px]">
            {framework}
          </span>
        )}

        {repo.last_onboarded_at && (
          <span className="ml-auto text-gray-7">
            onboarded {formatDistanceToNow(new Date(repo.last_onboarded_at), { addSuffix: true })}
          </span>
        )}

        {repo.cost_usd > 0 && (
          <span className="text-gray-7">${repo.cost_usd.toFixed(4)}</span>
        )}
      </div>

      {/* Error message */}
      {repo.status === 'failed' && repo.error_message && (
        <div className="mt-2 text-[12px] text-red-400 bg-red-400/10 rounded px-2.5 py-1.5 line-clamp-2">
          {repo.error_message}
        </div>
      )}
    </div>
  );
}
