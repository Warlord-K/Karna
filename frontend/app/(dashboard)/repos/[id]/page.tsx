'use client';

import { useParams, useRouter } from 'next/navigation';
import { RepoProfile, REPO_STATUS_COLORS, REPO_STATUS_LABELS } from '@/lib/repos';
import { useRepos, useDeleteRepo, useTriggerOnboard } from '@/hooks/use-repos';
import { ArrowLeft, ArrowsClockwise, Trash, GitBranch } from '@phosphor-icons/react';
import { MarkdownContent } from '@/components/agent/markdown-content';

export default function RepoDetailPage() {
  const params = useParams();
  const router = useRouter();
  const id = params.id as string;

  const { data: repos = [] } = useRepos();
  const repo = repos.find(r => r.id === id) ?? null;

  const deleteMutation = useDeleteRepo();
  const onboardMutation = useTriggerOnboard();

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
  const profile = repo.profile_json || {};

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-3xl mx-auto px-4 sm:px-6 py-6">
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
            <InfoItem label="Language" value={(profile.language as string) || '\u2014'} />
            <InfoItem label="Framework" value={(profile.framework as string) === 'null' ? '\u2014' : (profile.framework as string) || '\u2014'} />
            <InfoItem label="Package Manager" value={(profile.package_manager as string) || '\u2014'} />
            <InfoItem label="Cost" value={`$${repo.cost_usd.toFixed(4)}`} />
            {repo.last_commit_sha && (
              <InfoItem label="SHA" value={repo.last_commit_sha.slice(0, 8)} mono />
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
