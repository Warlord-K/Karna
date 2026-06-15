'use client';

import { useState } from 'react';
import { useRouter } from 'next/navigation';
import { useRepos, useAddRepo, useDeleteRepo, useTriggerOnboard } from '@/hooks/use-repos';
import { RepoCard } from '@/components/agent/repo-card';
import { AddRepoDialog } from '@/components/agent/add-repo-dialog';
import { Plus, GitFork } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import { PageHeader } from '@/components/ui/page-header';

export default function ReposListPage() {
  const router = useRouter();
  const [addOpen, setAddOpen] = useState(false);

  const { data: repos = [], isLoading } = useRepos();
  const addMutation = useAddRepo();
  const deleteMutation = useDeleteRepo();
  const onboardMutation = useTriggerOnboard();

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
      <div className="w-full px-4 sm:px-6 lg:px-8 py-6">
        <PageHeader
          title="Repos"
          description={
            repos.length === 0
              ? 'Add repositories to enable smart multi-repo planning'
              : `${readyCount} profiled${onboardingCount > 0 ? `, ${onboardingCount} onboarding` : ''}`
          }
          actions={
            <Button variant="primary" size="md" onClick={() => setAddOpen(true)}>
              <Plus size={15} weight="bold" />
              <span className="hidden sm:inline">Add Repo</span>
            </Button>
          }
        />

        {repos.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-20 text-gray-8">
            <GitFork size={48} weight="thin" className="mb-4" />
            <p className="text-[15px] font-medium text-gray-10">No repos onboarded</p>
            <p className="text-[13px] mt-1.5 max-w-xs text-center">
              Add your repositories so the agent can build profiles and route multi-repo tasks intelligently — no more exploring every repo for every task.
            </p>
            <Button variant="primary" size="lg" onClick={() => setAddOpen(true)} className="mt-4">
              <Plus size={15} weight="bold" /> Add repo
            </Button>
          </div>
        ) : (
          <div className="space-y-2">
            {repos.map((repo) => (
              <RepoCard
                key={repo.id}
                repo={repo}
                onClick={() => router.push(`/repos/${repo.id}`)}
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
    </div>
  );
}
