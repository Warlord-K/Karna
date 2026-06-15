'use client';

import Link from 'next/link';
import { useRouter } from 'next/navigation';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useSession } from 'next-auth/react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { formatDistanceToNow } from 'date-fns';
import {
  Bug,
  ChatCenteredText,
  ChatTeardropText,
  CircleNotch,
  ClockCounterClockwise,
  FolderOpen,
  ListChecks,
  PaperPlaneTilt,
  Sparkle,
} from '@phosphor-icons/react';
import toast from 'react-hot-toast';

import { useAuthDisabled } from '@/lib/auth-context';
import { useConfig } from '@/hooks/use-tasks';
import { chatStatusBadge, createChat, fetchChats } from '@/lib/chats';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

const chatsKey = ['chats', 'list'] as const;

interface SuggestedPrompt {
  label: string;
  icon: typeof Sparkle;
  /** Prefilled into the composer; trailing space invites the user to finish the thought. */
  text: string;
}

const SUGGESTED_PROMPTS: SuggestedPrompt[] = [
  {
    label: 'Summarize recent activity',
    icon: ClockCounterClockwise,
    text: 'Summarize recent activity and surface anything that needs attention.',
  },
  {
    label: 'Investigate an error',
    icon: Bug,
    text: 'Investigate an error I am seeing: ',
  },
  {
    label: 'Draft a plan for…',
    icon: ListChecks,
    text: 'Draft a plan for ',
  },
];

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

  const applyPrompt = useCallback((text: string) => {
    setMessage(text);
    requestAnimationFrame(() => {
      const node = composerRef.current;
      if (!node) return;
      node.focus();
      // Drop the caret at the end so the user can keep typing immediately.
      const end = node.value.length;
      node.setSelectionRange(end, end);
    });
  }, []);

  return (
    <div className="h-full overflow-y-auto">
      <div className="px-4 sm:px-6 py-8 sm:py-12">
        <div className="mx-auto w-full max-w-[720px] animate-fade-in-up">
          <div className="text-center mb-6">
            <div className="mx-auto mb-4 h-12 w-12 rounded-2xl border border-gray-4 bg-gray-2 flex items-center justify-center text-sun-9">
              <ChatTeardropText size={22} weight="bold" />
            </div>
            <h1 className="text-[24px] font-semibold text-gray-12 tracking-[-0.02em]">
              What can Karna help with?
            </h1>
            <p className="text-[13px] text-gray-8 mt-1.5 max-w-[440px] mx-auto">
              Ask a question or describe a task. Karna spins up an orchestrator task and streams progress live.
            </p>
          </div>

          <form onSubmit={handleStartChat}>
            <div className="rounded-2xl border border-gray-4 bg-gray-1 transition-smooth focus-within:border-gray-6 focus-within:ring-2 focus-within:ring-sun-9/25">
              <textarea
                ref={composerRef}
                value={message}
                onChange={(e) => setMessage(e.target.value)}
                onKeyDown={handleComposerKeyDown}
                rows={3}
                placeholder="Ask Karna anything…"
                className="w-full max-h-[260px] bg-transparent px-4 pt-3.5 text-[15px] leading-6 text-gray-12 placeholder:text-gray-7 resize-none focus:outline-none"
              />
              <div className="flex items-center gap-2 px-3 pb-3 pt-1">
                <div className="relative">
                  <FolderOpen
                    size={14}
                    weight="bold"
                    className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-7"
                  />
                  <select
                    value={repo}
                    onChange={(e) => setRepo(e.target.value)}
                    aria-label="Repository scope"
                    className="h-8 pl-8 pr-3 rounded-lg bg-gray-2 border border-gray-4 text-[12px] text-gray-11 transition-smooth focus-ring focus-visible:border-gray-6 max-w-[180px] truncate"
                  >
                    <option value="">No repo scope</option>
                    {repos.map((repoName) => (
                      <option key={repoName} value={repoName}>
                        {repoName}
                      </option>
                    ))}
                  </select>
                </div>
                <p className="hidden sm:block text-[11px] text-gray-7">
                  <kbd className="font-sans">Enter</kbd> to start
                </p>
                <Button
                  type="submit"
                  variant="primary"
                  size="md"
                  disabled={createMutation.isPending || !message.trim()}
                  className="ml-auto"
                >
                  {createMutation.isPending ? (
                    <>
                      <CircleNotch size={14} weight="bold" className="animate-spin" />
                      Starting…
                    </>
                  ) : (
                    <>
                      <PaperPlaneTilt size={14} weight="bold" />
                      Start chat
                    </>
                  )}
                </Button>
              </div>
            </div>
          </form>

          <div className="mt-4 flex flex-wrap items-center justify-center gap-2">
            {SUGGESTED_PROMPTS.map((prompt) => {
              const Icon = prompt.icon;
              return (
                <button
                  key={prompt.label}
                  type="button"
                  onClick={() => applyPrompt(prompt.text)}
                  className="inline-flex items-center gap-1.5 rounded-full border border-gray-4 bg-gray-2 px-3 py-1.5 text-[12px] text-gray-10 transition-smooth press-scale focus-ring hover:border-gray-5 hover:bg-gray-3 hover:text-gray-12"
                >
                  <Icon size={13} weight="bold" className="text-sun-9" />
                  {prompt.label}
                </button>
              );
            })}
          </div>
        </div>

        <div className="mx-auto w-full max-w-5xl mt-12">
          <div className="flex items-center gap-2 mb-3 px-1">
            <ChatTeardropText size={14} weight="bold" className="text-gray-8" />
            <p className="text-[11px] font-medium uppercase tracking-wider text-gray-8">Recent conversations</p>
          </div>

          {isLoading ? (
            <div className="rounded-xl border border-gray-4 bg-gray-2 p-6 text-[13px] text-gray-8 flex items-center gap-2">
              <CircleNotch size={14} weight="bold" className="animate-spin" />
              Loading conversations…
            </div>
          ) : chats.length === 0 ? (
            <div className="rounded-xl border border-dashed border-gray-4 bg-gray-2/50 p-10 text-center text-gray-8">
              <ChatCenteredText size={26} weight="thin" className="mx-auto mb-2" />
              <p className="text-[14px] text-gray-11">No conversations yet</p>
              <p className="text-[12px] mt-1">Start one with the composer above.</p>
            </div>
          ) : (
            <div className="rounded-xl border border-gray-4 bg-gray-2 overflow-hidden divide-y divide-gray-4">
              {chats.map((chat) => {
                const badge = chatStatusBadge(chat.status);
                return (
                  <Link
                    key={chat.id}
                    href={`/chat/${chat.id}`}
                    className="group flex items-center gap-3 px-4 py-3 transition-smooth hover:bg-gray-3/60 focus-ring"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <p className="text-[14px] text-gray-12 font-medium truncate group-hover:text-gray-12">
                          {chat.title}
                        </p>
                        <Badge tone={badge.tone} className="flex-shrink-0">{badge.label}</Badge>
                      </div>
                      <p className="text-[12px] text-gray-8 mt-0.5 truncate">
                        {chat.description || 'No description'}
                      </p>
                    </div>
                    <div className="flex flex-col items-end gap-1 flex-shrink-0">
                      {chat.repo && (
                        <span className="text-[11px] text-gray-7 font-mono truncate max-w-[140px]">{chat.repo}</span>
                      )}
                      <span className="text-[11px] text-gray-7">
                        {chat.updated_at ? formatDistanceToNow(new Date(chat.updated_at), { addSuffix: true }) : ''}
                      </span>
                    </div>
                  </Link>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
