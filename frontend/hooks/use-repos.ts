import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  RepoProfile,
  fetchRepos,
  addRepo,
  deleteRepo,
  triggerOnboard,
} from '@/lib/repos';
import toast from 'react-hot-toast';

export const repoKeys = {
  all: ['repos'] as const,
  lists: () => [...repoKeys.all, 'list'] as const,
};

export function useRepos() {
  return useQuery<RepoProfile[]>({
    queryKey: repoKeys.lists(),
    queryFn: ({ signal }) => fetchRepos(signal),
    refetchInterval: 5000,
  });
}

export function useAddRepo() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (data: { repo: string; branch: string }) => addRepo(data),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: repoKeys.lists() });
      toast.success('Repo added — onboarding will start shortly');
    },
    onError: () => toast.error('Failed to add repo'),
  });
}

export function useDeleteRepo() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteRepo(id),
    onMutate: async (id) => {
      await qc.cancelQueries({ queryKey: repoKeys.lists() });
      const prev = qc.getQueryData<RepoProfile[]>(repoKeys.lists());
      if (prev) {
        qc.setQueryData<RepoProfile[]>(
          repoKeys.lists(),
          prev.filter(r => r.id !== id)
        );
      }
      return { prev };
    },
    onSuccess: () => toast.success('Repo removed'),
    onError: (_err, _vars, ctx) => {
      if (ctx?.prev) qc.setQueryData(repoKeys.lists(), ctx.prev);
      toast.error('Failed to remove repo');
    },
    onSettled: () => qc.invalidateQueries({ queryKey: repoKeys.lists() }),
  });
}

export function useTriggerOnboard() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => triggerOnboard(id),
    onMutate: async (id) => {
      await qc.cancelQueries({ queryKey: repoKeys.lists() });
      const prev = qc.getQueryData<RepoProfile[]>(repoKeys.lists());
      if (prev) {
        qc.setQueryData<RepoProfile[]>(
          repoKeys.lists(),
          prev.map(r => (r.id === id ? { ...r, status: 'pending' as const } : r))
        );
      }
      return { prev };
    },
    onSuccess: () => toast.success('Re-onboarding triggered'),
    onError: (_err, _vars, ctx) => {
      if (ctx?.prev) qc.setQueryData(repoKeys.lists(), ctx.prev);
      toast.error('Failed to trigger onboarding');
    },
    onSettled: () => qc.invalidateQueries({ queryKey: repoKeys.lists() }),
  });
}
