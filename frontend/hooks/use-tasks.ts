import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  AgentTask,
  AgentLog,
  AgentTaskPriority,
  fetchTasks,
  createTask,
  updateTask,
  deleteTask,
  fetchLogs,
  fetchSubtasks,
  approveWithSubtasks,
  postComment,
  nestSubtasks,
} from '@/lib/agent-tasks';
import toast from 'react-hot-toast';

export const taskKeys = {
  all: ['tasks'] as const,
  lists: () => [...taskKeys.all, 'list'] as const,
  subtasks: (taskId: string) => [...taskKeys.all, 'subtasks', taskId] as const,
  logs: (taskId: string) => [...taskKeys.all, 'logs', taskId] as const,
  config: ['config'] as const,
};

interface AppConfig {
  repos: string[];
  backends: Record<string, { models: string[]; default_model: string }>;
  skills: string[];
  mcpServers: string[];
}

export function useConfig(enabled: boolean) {
  return useQuery<AppConfig>({
    queryKey: taskKeys.config,
    queryFn: async () => {
      const res = await fetch('/api/config');
      const data = await res.json();
      return {
        repos: (data.repos || []).map((r: any) => r.repo),
        backends: data.backends || {},
        skills: data.skills || [],
        mcpServers: data.mcpServers || [],
      };
    },
    enabled,
    staleTime: 60_000,
  });
}

export function useTasks(enabled: boolean) {
  return useQuery<AgentTask[]>({
    queryKey: taskKeys.lists(),
    queryFn: async () => {
      const data = await fetchTasks();
      return nestSubtasks(data);
    },
    enabled,
    refetchInterval: 5000,
  });
}

export function useSubtasks(taskId: string | null, poll: boolean) {
  return useQuery<AgentTask[]>({
    queryKey: taskKeys.subtasks(taskId!),
    queryFn: () => fetchSubtasks(taskId!),
    enabled: !!taskId,
    refetchInterval: poll ? 5000 : false,
  });
}

export function useLogs(taskId: string | null, poll: boolean) {
  return useQuery<AgentLog[]>({
    queryKey: taskKeys.logs(taskId!),
    queryFn: () => fetchLogs(taskId!),
    enabled: !!taskId && poll,
    refetchInterval: poll ? 3000 : false,
  });
}

export function useCreateTask() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (data: {
      title: string;
      description: string;
      repo: string | null;
      priority: AgentTaskPriority;
      cli: string | null;
      model: string | null;
    }) => createTask(data),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: taskKeys.lists() });
      toast.success('Task created');
    },
    onError: () => toast.error('Failed to create task'),
  });
}

export function useUpdateTask() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, updates }: { id: string; updates: Partial<AgentTask> }) =>
      updateTask(id, updates),
    onMutate: async ({ id, updates }) => {
      await qc.cancelQueries({ queryKey: taskKeys.lists() });
      const prev = qc.getQueryData<AgentTask[]>(taskKeys.lists());
      if (prev) {
        qc.setQueryData<AgentTask[]>(
          taskKeys.lists(),
          prev.map(t => (t.id === id ? { ...t, ...updates } : t))
        );
      }
      return { prev };
    },
    onError: (_err, _vars, ctx) => {
      if (ctx?.prev) qc.setQueryData(taskKeys.lists(), ctx.prev);
      toast.error('Failed to update task');
    },
    onSettled: () => qc.invalidateQueries({ queryKey: taskKeys.lists() }),
  });
}

export function useDeleteTask() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteTask(id),
    onMutate: async (id) => {
      await qc.cancelQueries({ queryKey: taskKeys.lists() });
      const prev = qc.getQueryData<AgentTask[]>(taskKeys.lists());
      if (prev) {
        qc.setQueryData<AgentTask[]>(
          taskKeys.lists(),
          prev.filter(t => t.id !== id)
        );
      }
      return { prev };
    },
    onSuccess: () => toast.success('Task deleted'),
    onError: (_err, _vars, ctx) => {
      if (ctx?.prev) qc.setQueryData(taskKeys.lists(), ctx.prev);
      toast.error('Failed to delete task');
    },
    onSettled: () => qc.invalidateQueries({ queryKey: taskKeys.lists() }),
  });
}

export function useApproveWithSubtasks() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (taskId: string) => approveWithSubtasks(taskId),
    onSuccess: (_data, taskId) => {
      qc.invalidateQueries({ queryKey: taskKeys.subtasks(taskId) });
      qc.invalidateQueries({ queryKey: taskKeys.lists() });
    },
  });
}

export function usePostComment() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ taskId, message }: { taskId: string; message: string }) =>
      postComment(taskId, message),
    onSuccess: (_data, { taskId }) => {
      qc.invalidateQueries({ queryKey: taskKeys.logs(taskId) });
    },
  });
}
