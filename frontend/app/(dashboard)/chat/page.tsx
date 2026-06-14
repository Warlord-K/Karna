'use client';

import Link from 'next/link';
import { useRouter } from 'next/navigation';
import { useState } from 'react';
import { useSession } from 'next-auth/react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { formatDistanceToNow } from 'date-fns';
import { ChatCenteredText, ChatTeardropText, PaperPlaneTilt } from '@phosphor-icons/react';
import toast from 'react-hot-toast';

import { useAuthDisabled } from '@/lib/auth-context';
import { useConfig } from '@/hooks/use-tasks';
import { createChat, fetchChats } from '@/lib/chats';

const chatsKey = ['chats', 'list'] as const;

export default function ChatIndexPage() {
  const router = useRouter();
  const queryClient = useQueryClient();
  const authDisabled = useAuthDisabled();
  const { status: authStatus } = useSession();
  const isReady = authDisabled || authStatus === 'authenticated';

  const { data: config } = useConfig(isReady);
  const repos = config?.repos ?? [];

  const [message, setMessage] = useState('');
  const [repo, setRepo] = useState('');

  const { data: chats = [], isLoading } = useQuery({
    queryKey: chatsKey,
    queryFn: ({ signal }) => fetchChats(signal),
    enabled: isReady,
    refetchInterval: 5000,
  });

  const createMutation = useMutation({
    mutationFn: async () =>
      createChat({
        message,
        repo: repo || null,
      }),
    onSuccess: async (chat) => {
      setMessage('');
      setRepo('');
      await queryClient.invalidateQueries({ queryKey: chatsKey });
      router.push(`/chat/${chat.id}`);
    },
    onError: () => {
      toast.error('Failed to create chat');
    },
  });

  const handleStartChat = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!message.trim()) return;
    await createMutation.mutateAsync();
  };

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-5xl mx-auto px-4 sm:px-6 py-6 space-y-6">
        <div>
          <h1 className="text-[22px] font-semibold text-gray-12 tracking-[-0.02em]">Chat</h1>
          <p className="text-[13px] text-gray-8 mt-1">
            Start a conversation with Karna. Each chat runs as an orchestrator task.
          </p>
        </div>

        <form onSubmit={handleStartChat} className="rounded-xl border border-gray-4 bg-gray-2 p-4 space-y-3">
          <textarea
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            rows={4}
            placeholder="Ask Karna anything..."
            className="w-full px-3 py-2 rounded-lg bg-gray-1 border border-gray-4 text-[14px] text-gray-12 placeholder:text-gray-7 focus:outline-none focus:border-gray-6 resize-y"
          />
          <div className="flex items-center gap-2">
            <select
              value={repo}
              onChange={(e) => setRepo(e.target.value)}
              className="h-9 px-3 rounded-lg bg-gray-1 border border-gray-4 text-[13px] text-gray-11 focus:outline-none focus:border-gray-6"
            >
              <option value="">No repo scope</option>
              {repos.map((repoName) => (
                <option key={repoName} value={repoName}>
                  {repoName}
                </option>
              ))}
            </select>
            <button
              type="submit"
              disabled={createMutation.isPending || !message.trim()}
              className="ml-auto h-9 px-4 rounded-lg bg-sun-9 hover:bg-sun-10 text-gray-1 text-[13px] font-medium disabled:opacity-50 disabled:cursor-not-allowed inline-flex items-center gap-1.5"
            >
              <PaperPlaneTilt size={14} weight="bold" />
              {createMutation.isPending ? 'Starting...' : 'New chat'}
            </button>
          </div>
        </form>

        <div className="rounded-xl border border-gray-4 bg-gray-2 divide-y divide-gray-4">
          {isLoading ? (
            <div className="p-6 text-[13px] text-gray-8">Loading conversations...</div>
          ) : chats.length === 0 ? (
            <div className="p-10 text-center text-gray-8">
              <ChatCenteredText size={28} weight="thin" className="mx-auto mb-2" />
              <p className="text-[14px]">No conversations yet</p>
            </div>
          ) : (
            chats.map((chat) => (
              <Link
                key={chat.id}
                href={`/chat/${chat.id}`}
                className="block px-4 py-3 hover:bg-gray-3/60 transition-colors"
              >
                <div className="flex items-center gap-2">
                  <ChatTeardropText size={14} weight="bold" className="text-gray-8" />
                  <span className="text-[14px] text-gray-12 font-medium truncate">{chat.title}</span>
                  <span className="ml-auto text-[11px] text-gray-7 uppercase tracking-wide">{chat.status}</span>
                </div>
                <p className="text-[12px] text-gray-8 mt-1 truncate">
                  {chat.description || 'No description'}
                </p>
                {chat.created_at && (
                  <p className="text-[11px] text-gray-7 mt-1">
                    {formatDistanceToNow(new Date(chat.created_at), { addSuffix: true })}
                  </p>
                )}
              </Link>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
