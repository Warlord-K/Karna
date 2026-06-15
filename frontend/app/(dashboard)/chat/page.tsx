'use client';

import Link from 'next/link';
import { useRouter } from 'next/navigation';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useSession } from 'next-auth/react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { formatDistanceToNow } from 'date-fns';
import { ChatCenteredText, ChatTeardropText, CircleNotch, PaperPlaneTilt } from '@phosphor-icons/react';
import toast from 'react-hot-toast';

import { useAuthDisabled } from '@/lib/auth-context';
import { useConfig } from '@/hooks/use-tasks';
import { createChat, fetchChats } from '@/lib/chats';
import { Button } from '@/components/ui/button';

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
  const composerRef = useRef<HTMLTextAreaElement>(null);

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

  const resizeComposer = useCallback(() => {
    const node = composerRef.current;
    if (!node) return;
    node.style.height = '0px';
    node.style.height = `${Math.min(node.scrollHeight, 220)}px`;
  }, []);

  useEffect(() => {
    resizeComposer();
  }, [message, resizeComposer]);

  const handleStartChat = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!message.trim()) return;
    await createMutation.mutateAsync();
  };

  const handleComposerKeyDown = async (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      if (!message.trim() || createMutation.isPending) return;
      await createMutation.mutateAsync();
    }
  };

  const statusClassName = (status: string) => {
    if (status === 'done') return 'text-green-400';
    if (status === 'failed') return 'text-red-400';
    if (status === 'cancelled') return 'text-gray-7';
    if (status === 'review') return 'text-blue-300';
    if (status === 'planning' || status === 'in_progress') return 'text-sun-9';
    return 'text-gray-8';
  };

  return (
    <div className="h-full overflow-y-auto">
      <div className="px-4 sm:px-6 py-6 space-y-6">
        <div>
          <h1 className="text-[22px] font-semibold text-gray-12 tracking-[-0.02em]">Chat</h1>
          <p className="text-[13px] text-gray-8 mt-1">
            Karna conversations are backed by orchestrator tasks with live streaming updates.
          </p>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-[320px_1fr] gap-4">
          <section className="rounded-xl border border-gray-4 bg-gray-2 overflow-hidden min-h-[360px]">
            <div className="px-4 py-3 border-b border-gray-4 flex items-center gap-2">
              <ChatTeardropText size={14} weight="bold" className="text-gray-8" />
              <p className="text-[12px] uppercase tracking-wider text-gray-8">Conversations</p>
            </div>
            <div className="divide-y divide-gray-4">
              {isLoading ? (
                <div className="p-6 text-[13px] text-gray-8 flex items-center gap-2">
                  <CircleNotch size={14} weight="bold" className="animate-spin" />
                  Loading conversations...
                </div>
              ) : chats.length === 0 ? (
                <div className="p-10 text-center text-gray-8">
                  <ChatCenteredText size={28} weight="thin" className="mx-auto mb-2" />
                  <p className="text-[14px]">No conversations yet</p>
                  <p className="text-[12px] mt-1">Start one from the panel on the right.</p>
                </div>
              ) : (
                chats.map((chat) => (
                  <Link
                    key={chat.id}
                    href={`/chat/${chat.id}`}
                    className="block px-4 py-3 hover:bg-gray-3/60 transition-colors"
                  >
                    <div className="flex items-center gap-2">
                      <span className={`text-[11px] font-medium uppercase tracking-wide ${statusClassName(chat.status)}`}>
                        {chat.status}
                      </span>
                      <span className="text-gray-6">•</span>
                      <span className="text-[11px] text-gray-7">
                        {chat.updated_at ? formatDistanceToNow(new Date(chat.updated_at), { addSuffix: true }) : ''}
                      </span>
                    </div>
                    <p className="text-[14px] text-gray-12 font-medium truncate mt-1">{chat.title}</p>
                    <p className="text-[12px] text-gray-8 mt-1 truncate">
                      {chat.description || 'No description'}
                    </p>
                  </Link>
                ))
              )}
            </div>
          </section>

          <section className="rounded-xl border border-gray-4 bg-gray-2 p-4 sm:p-5">
            <div className="mb-4">
              <p className="text-[15px] font-medium text-gray-12">New chat</p>
              <p className="text-[12px] text-gray-8 mt-1">
                Ask a question, optionally scope to a repo, and Karna will create a new chat task.
              </p>
            </div>

            <form onSubmit={handleStartChat} className="space-y-3">
              <textarea
                ref={composerRef}
                value={message}
                onChange={(e) => setMessage(e.target.value)}
                onKeyDown={handleComposerKeyDown}
                rows={2}
                placeholder="Ask Karna anything..."
                className="w-full max-h-[220px] px-3 py-2 rounded-lg bg-gray-1 border border-gray-4 text-[14px] text-gray-12 placeholder:text-gray-7 transition-smooth focus-ring focus-visible:border-gray-6 resize-none"
              />
              <div className="flex flex-col sm:flex-row sm:items-center gap-2">
                <select
                  value={repo}
                  onChange={(e) => setRepo(e.target.value)}
                  className="h-9 px-3 rounded-lg bg-gray-1 border border-gray-4 text-[13px] text-gray-11 transition-smooth focus-ring focus-visible:border-gray-6"
                >
                  <option value="">No repo scope</option>
                  {repos.map((repoName) => (
                    <option key={repoName} value={repoName}>
                      {repoName}
                    </option>
                  ))}
                </select>
                <Button
                  type="submit"
                  variant="primary"
                  size="lg"
                  disabled={createMutation.isPending || !message.trim()}
                  className="sm:ml-auto"
                >
                  {createMutation.isPending ? (
                    <>
                      <CircleNotch size={14} weight="bold" className="animate-spin" />
                      Starting...
                    </>
                  ) : (
                    <>
                      <PaperPlaneTilt size={14} weight="bold" />
                      New chat
                    </>
                  )}
                </Button>
              </div>
              <p className="text-[11px] text-gray-7">Enter to start, Shift+Enter for newline.</p>
            </form>
          </section>
        </div>
      </div>
    </div>
  );
}
