import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  TaskAttachment,
  fetchAttachments,
  uploadAttachment,
  deleteAttachment,
} from '@/lib/agent-tasks';
import toast from 'react-hot-toast';

export const attachmentKeys = {
  all: ['attachments'] as const,
  list: (taskId: string) => [...attachmentKeys.all, taskId] as const,
};

export function useAttachments(taskId: string | null) {
  return useQuery<TaskAttachment[]>({
    queryKey: attachmentKeys.list(taskId!),
    queryFn: () => fetchAttachments(taskId!),
    enabled: !!taskId,
    staleTime: 30_000,
  });
}

export function useUploadAttachment(taskId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (file: File) => uploadAttachment(taskId, file),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: attachmentKeys.list(taskId) });
    },
    onError: (err: Error) => {
      toast.error(err.message || 'Failed to upload image');
    },
  });
}

export function useDeleteAttachment(taskId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (attachmentId: string) => deleteAttachment(taskId, attachmentId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: attachmentKeys.list(taskId) });
      toast.success('Image removed');
    },
    onError: () => {
      toast.error('Failed to remove image');
    },
  });
}
