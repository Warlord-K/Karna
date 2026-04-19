'use client';

import { RepoProfile, REPO_STATUS_COLORS, REPO_STATUS_LABELS } from '@/lib/repos';
import { useUpdateRepo } from '@/hooks/use-repos';
import { X, ArrowsClockwise, Trash, GitBranch } from '@phosphor-icons/react';
import { MarkdownContent } from './markdown-content';

interface RepoDetailModalProps {
  repo: RepoProfile | null;
  onClose: () => void;
  onOnboard: (id: string) => void;
  onDelete: (id: string) => void;
}

export function RepoDetailModal({ repo, onClose, onOnboard, onDelete }: RepoDetailModalProps) {
  const updateMutation = useUpdateRepo();

  if (!repo) return null;

  const statusColor = REPO_STATUS_COLORS[repo.status];
  const statusLabel = REPO_STATUS_LABELS[repo.status];
  const profile = repo.profile_json || {};

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={onClose}>
      <div
        className="bg-gray-1 border border-gray-3 rounded-xl shadow-elevated w-full max-w-2xl mx-4 max-h-[85vh] flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-gray-3 flex-shrink-0">
          <div className="flex items-center gap-3 min-w-0">
            <span
              className="w-2.5 h-2.5 rounded-full flex-shrink-0"
              style={{ backgroundColor: statusColor }}
            />
            <h2 className="text-[16px] font-semibold text-gray-12 truncate font-mono">{repo.repo}</h2>
            <span
              className="px-2 py-0.5 rounded text-[11px] font-medium flex-shrink-0"
              style={{ backgroundColor: statusColor + '20', color: statusColor }}
            >
              {statusLabel}
            </span>
          </div>
          <div className="flex items-center gap-1 flex-shrink-0">
            <button
              onClick={() => onOnboard(repo.id)}
              disabled={repo.status === 'onboarding'}
              title="Re-onboard"
              className="h-8 w-8 flex items-center justify-center text-gray-8 hover:text-sun-9 hover:bg-gray-3 rounded-lg transition-colors disabled:opacity-40"
            >
              <ArrowsClockwise size={16} weight="bold" className={repo.status === 'onboarding' ? 'animate-spin' : ''} />
            </button>
            <button
              onClick={() => { onDelete(repo.id); onClose(); }}
              title="Delete"
              className="h-8 w-8 flex items-center justify-center text-gray-8 hover:text-red-400 hover:bg-gray-3 rounded-lg transition-colors"
            >
              <Trash size={16} weight="bold" />
            </button>
            <button onClick={onClose} className="h-8 w-8 flex items-center justify-center text-gray-8 hover:text-gray-12 transition-colors">
              <X size={18} weight="bold" />
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-5">
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
              <div className="prose prose-invert prose-sm max-w-none text-[13px] text-gray-9">
                <MarkdownContent content={repo.summary} />
              </div>
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
    </div>
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
