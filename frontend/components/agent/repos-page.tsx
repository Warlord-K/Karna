'use client';

import { useState } from 'react';
import { RepoProfile } from '@/lib/repos';
import { useRepos, useAddRepo, useDeleteRepo, useTriggerOnboard } from '@/hooks/use-repos';
import { RepoCard } from './repo-card';
import { AddRepoDialog } from './add-repo-dialog';
import { RepoDetailModal } from './repo-detail-modal';
import { Plus, GitFork } from '@phosphor-icons/react';

export function ReposPage() {
  const [addOpen, setAddOpen] = useState(false);
  const [selectedRepo, setSelectedRepo] = useState<RepoProfile | null>(null);

  const { data: repos = [], isLoading } = useRepos();
  const addMutation = useAddRepo();
  const deleteMutation = useDeleteRepo();
  const onboardMutation = useTriggerOnboard();

  // Keep selected repo in sync with query data
  const selectedRepoData = selectedRepo
    ? repos.find(r => r.id === selectedRepo.id) ?? selectedRepo
    : null;

  const handleAdd = async (data: { repo: string; branch: string }) => {
    await addMutation.mutateAsync(data);
  };

  const handleDelete = async (id: string) => {
    await deleteMutation.mutateAsync(id);
  };

  const handleOnboard = async (id: string) => {
    await onboardMutation.mutateAsync(id);
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="w-5 h-5 border-2 border-gray-7 border-t-gray-12 rounded-full animate-spin" />
      </div>
    );
  }

  const readyCount = repos.filter(r => r.status === 'ready').length;
  const onboardingCount = repos.filter(r => r.status === 'onboarding' || r.status === 'pending').length;

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-3xl mx-auto px-4 sm:px-6 py-6">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div>
            <h2 className="text-[18px] font-semibold text-gray-12 tracking-[-0.02em]">Repos</h2>
            <p className="text-[13px] text-gray-8 mt-0.5">
              {repos.length === 0
                ? 'Add repositories to enable smart multi-repo planning'
                : `${readyCount} profiled${onboardingCount > 0 ? `, ${onboardingCount} onboarding` : ''}`}
            </p>
          </div>
          <button
            onClick={() => setAddOpen(true)}
            className="h-8 sm:w-auto px-3.5 text-[13px] font-medium text-white bg-sun-9 hover:bg-sun-10 text-gray-1 rounded-lg transition-colors flex items-center gap-1.5"
          >
            <Plus size={15} weight="bold" />
            <span className="hidden sm:inline">Add Repo</span>
          </button>
        </div>

        {/* Repo list */}
        {repos.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-20 text-gray-8">
            <GitFork size={48} weight="thin" className="mb-4" />
            <p className="text-[15px] font-medium text-gray-10">No repos onboarded</p>
            <p className="text-[13px] mt-1.5 max-w-xs text-center">
              Add your repositories so the agent can build profiles and route multi-repo tasks intelligently — no more exploring every repo for every task.
            </p>
            <button
              onClick={() => setAddOpen(true)}
              className="h-9 px-4 mt-4 text-[14px] font-medium text-white bg-sun-9 hover:bg-sun-10 text-gray-1 rounded-lg transition-colors flex items-center gap-1.5"
            >
              <Plus size={15} weight="bold" /> Add repo
            </button>
          </div>
        ) : (
          <div className="space-y-2">
            {repos.map((repo) => (
              <RepoCard
                key={repo.id}
                repo={repo}
                onClick={() => setSelectedRepo(repo)}
                onOnboard={() => handleOnboard(repo.id)}
                onDelete={() => handleDelete(repo.id)}
              />
            ))}
          </div>
        )}
      </div>

      <AddRepoDialog
        open={addOpen}
        onClose={() => setAddOpen(false)}
        onAdd={handleAdd}
      />

      <RepoDetailModal
        repo={selectedRepoData}
        onClose={() => setSelectedRepo(null)}
        onOnboard={handleOnboard}
        onDelete={handleDelete}
      />
    </div>
  );
}
