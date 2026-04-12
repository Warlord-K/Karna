import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Schedule,
  ScheduledRun,
  ScheduledRunLog,
  fetchSchedules,
  createSchedule,
  updateSchedule,
  deleteSchedule,
  triggerSchedule,
  fetchRuns,
  fetchRunLogs,
} from '@/lib/schedules';
import toast from 'react-hot-toast';

export const scheduleKeys = {
  all: ['schedules'] as const,
  lists: () => [...scheduleKeys.all, 'list'] as const,
  runs: (scheduleId: string) => [...scheduleKeys.all, 'runs', scheduleId] as const,
  runLogs: (scheduleId: string, runId: string) =>
    [...scheduleKeys.all, 'runLogs', scheduleId, runId] as const,
};

export function useSchedules() {
  return useQuery<Schedule[]>({
    queryKey: scheduleKeys.lists(),
    queryFn: fetchSchedules,
    refetchInterval: 10_000,
  });
}

export function useScheduleRuns(scheduleId: string | null, poll: boolean) {
  return useQuery<ScheduledRun[]>({
    queryKey: scheduleKeys.runs(scheduleId!),
    queryFn: () => fetchRuns(scheduleId!),
    enabled: !!scheduleId,
    refetchInterval: poll ? 5000 : false,
  });
}

export function useScheduleRunLogs(
  scheduleId: string | null,
  runId: string | null,
  poll: boolean
) {
  return useQuery<ScheduledRunLog[]>({
    queryKey: scheduleKeys.runLogs(scheduleId!, runId!),
    queryFn: () => fetchRunLogs(scheduleId!, runId!),
    enabled: !!scheduleId && !!runId,
    refetchInterval: poll ? 3000 : false,
  });
}

export function useCreateSchedule() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (data: Parameters<typeof createSchedule>[0]) => createSchedule(data),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: scheduleKeys.lists() });
      toast.success('Schedule created');
    },
    onError: () => toast.error('Failed to create schedule'),
  });
}

export function useUpdateSchedule() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, updates }: { id: string; updates: Partial<Schedule> }) =>
      updateSchedule(id, updates),
    onMutate: async ({ id, updates }) => {
      await qc.cancelQueries({ queryKey: scheduleKeys.lists() });
      const prev = qc.getQueryData<Schedule[]>(scheduleKeys.lists());
      if (prev) {
        qc.setQueryData<Schedule[]>(
          scheduleKeys.lists(),
          prev.map(s => (s.id === id ? { ...s, ...updates } : s))
        );
      }
      return { prev };
    },
    onError: (_err, _vars, ctx) => {
      if (ctx?.prev) qc.setQueryData(scheduleKeys.lists(), ctx.prev);
      toast.error('Failed to update schedule');
    },
    onSettled: () => qc.invalidateQueries({ queryKey: scheduleKeys.lists() }),
  });
}

export function useDeleteSchedule() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteSchedule(id),
    onMutate: async (id) => {
      await qc.cancelQueries({ queryKey: scheduleKeys.lists() });
      const prev = qc.getQueryData<Schedule[]>(scheduleKeys.lists());
      if (prev) {
        qc.setQueryData<Schedule[]>(
          scheduleKeys.lists(),
          prev.filter(s => s.id !== id)
        );
      }
      return { prev };
    },
    onSuccess: () => toast.success('Schedule deleted'),
    onError: (_err, _vars, ctx) => {
      if (ctx?.prev) qc.setQueryData(scheduleKeys.lists(), ctx.prev);
      toast.error('Failed to delete schedule');
    },
    onSettled: () => qc.invalidateQueries({ queryKey: scheduleKeys.lists() }),
  });
}

export function useTriggerSchedule() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => triggerSchedule(id),
    onSuccess: (_data, id) => {
      qc.invalidateQueries({ queryKey: scheduleKeys.runs(id) });
      toast.success('Schedule triggered');
    },
    onError: () => toast.error('Failed to trigger schedule'),
  });
}
