'use client';

import Link from 'next/link';
import { useParams, useRouter } from 'next/navigation';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useSession } from 'next-auth/react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { formatDistanceToNow } from 'date-fns';
import { ArrowLeft, ChatCenteredText, CircleNotch, Lightning, PaperPlaneTilt } from '@phosphor-icons/react';
import toast from 'react-hot-toast';

import { useAuthDisabled } from '@/lib/auth-context';
import { fetchChats } from '@/lib/chats';
import {
  AgentLog,
  buildLogStreamUrl,
  encodeLogCursor,
  fetchLogs,
  postComment,
} from '@/lib/agent-tasks';
import { mergeTaskLogs } from '@/hooks/use-tasks';
import { TaskThreadView } from '@/components/agent/task-thread-view';
import { Button } from '@/components/ui/button';

const chatsKey = ['chats', 'list'] as const;

export default function ChatConversationPage() {
  const params = useParams();
  const router = useRouter();
  const queryClient = useQueryClient();
  const chatId = params.id as string;

  const authDisabled = useAuthDisabled();
  const { status: authStatus } = useSession();
  const isReady = authDisabled || authStatus === 'authenticated';

  const [message, setMessage] = useState('');
  const [streamLogs, setStreamLogs] = useState<AgentLog[]>([]);
  const [streamStatus, setStreamStatus] = useState<'idle' | 'connecting' | 'open' | 'fallback'>('idle');
  const messagesRef = useRef<HTMLDivElement>(null);
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const shouldStickToBottomRef = useRef(true);
  const fallbackNotifiedRef = useRef(false);
  const [showJumpToLatest, setShowJumpToLatest] = useState(false);

  const { data: chats = [], isLoading: chatsLoading } = useQuery({
    queryKey: chatsKey,
    queryFn: ({ signal }) => fetchChats(signal),
    enabled: isReady,
    refetchInterval: 5000,
  });
  const chat = chats.find((item) => item.id === chatId) ?? null;

  const shouldPollLogs = streamStatus !== 'open';
  const { data: polledLogs = [], isLoading: logsLoading } = useQuery({
    queryKey: ['tasks', 'logs', chatId],
    queryFn: ({ signal }) => fetchLogs(chatId, signal),
    enabled: isReady && !!chatId,
    refetchInterval: shouldPollLogs ? 3000 : false,
  });

  const logs = useMemo(
    () => mergeTaskLogs(polledLogs, streamLogs),
    [polledLogs, streamLogs],
  );

  const updateScrollState = useCallback(() => {
    const node = messagesRef.current;
    if (!node) return;

    const distanceFromBottom = node.scrollHeight - node.scrollTop - node.clientHeight;
    const isNearBottom = distanceFromBottom < 120;
    shouldStickToBottomRef.current = isNearBottom;
    setShowJumpToLatest(!isNearBottom);
  }, []);

  const scrollToBottom = useCallback((behavior: ScrollBehavior = 'smooth') => {
    const node = messagesRef.current;
    if (!node) return;
    node.scrollTo({ top: node.scrollHeight, behavior });
    shouldStickToBottomRef.current = true;
    setShowJumpToLatest(false);
  }, []);

  const resizeComposer = useCallback(() => {
    const node = composerRef.current;
    if (!node) return;
    node.style.height = '0px';
    node.style.height = `${Math.min(node.scrollHeight, 220)}px`;
  }, []);

  useEffect(() => {
    resizeComposer();
  }, [message, resizeComposer]);

  useEffect(() => {
    if (!logs.length) return;
    if (shouldStickToBottomRef.current) {
      scrollToBottom(logs.length < 2 ? 'auto' : 'smooth');
    }
  }, [logs.length, scrollToBottom]);

  useEffect(() => {
    setStreamLogs([]);
    setStreamStatus('idle');
    shouldStickToBottomRef.current = true;
    fallbackNotifiedRef.current = false;
    setShowJumpToLatest(false);
  }, [chatId]);

  useEffect(() => {
    if (!chatId) return;
    if (typeof window === 'undefined' || typeof window.EventSource === 'undefined') {
      setStreamStatus('fallback');
      return;
    }

    setStreamStatus('connecting');
    const source = new EventSource(buildLogStreamUrl(chatId, encodeLogCursor(logs[logs.length - 1])));
    let closed = false;

    const onLogData = (raw: string) => {
      try {
        const next = JSON.parse(raw) as AgentLog;
        if (!next?.id) return;
        setStreamLogs((current) => mergeTaskLogs(current, [next]));
      } catch {
        // Ignore malformed events and keep stream alive.
      }
    };

    source.addEventListener('log', (event: MessageEvent) => onLogData(event.data));
    source.onmessage = (event) => onLogData(event.data);
    source.onopen = () => {
      fallbackNotifiedRef.current = false;
      setStreamStatus('open');
    };
    source.onerror = () => {
      if (closed) return;
      closed = true;
      source.close();
      setStreamStatus('fallback');
      if (!fallbackNotifiedRef.current) {
        fallbackNotifiedRef.current = true;
        toast.error('Live stream disconnected. Switched to polling.');
      }
    };

    return () => {
      closed = true;
      source.close();
    };
    // Reconnect when switching chats.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [chatId]);

  const commentMutation = useMutation({
    mutationFn: async (trimmedMessage: string) => postComment(chatId, trimmedMessage),
    onSuccess: async () => {
      setMessage('');
      requestAnimationFrame(() => resizeComposer());
      await queryClient.invalidateQueries({ queryKey: ['tasks', 'logs', chatId] });
      scrollToBottom('smooth');
    },
    onError: () => {
      toast.error('Failed to send message');
    },
  });

  const handleSend = useCallback(async () => {
    const trimmed = message.trim();
    if (!trimmed || commentMutation.isPending) return;
    try {
      await commentMutation.mutateAsync(trimmed);
    } catch {
      // Handled by mutation onError.
    }
  }, [commentMutation, message]);

  const handleComposerSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    await handleSend();
  };

  const handleComposerKeyDown = async (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      await handleSend();
    }
  };

  if (chatsLoading && !chat) {
    return (
      <div className="h-full flex items-center justify-center text-gray-8 text-[14px]">
        Loading conversation...
      </div>
    );
  }

  if (!chat) {
    return (
      <div className="h-full flex flex-col items-center justify-center text-gray-8">
        <ChatCenteredText size={28} weight="thin" className="mb-2" />
        <p className="text-[14px]">Chat not found</p>
        <Link href="/chat" className="mt-3 text-[13px] text-sun-10 hover:text-sun-9">
          Back to chats
        </Link>
      </div>
    );
  }

  const isAgentWorking = chat.status === 'planning' || chat.status === 'in_progress';
  const showTypingIndicator = isAgentWorking && (streamStatus === 'open' || streamStatus === 'connecting');

  return (
    <div className="h-full overflow-hidden">
      <div className="h-full px-4 sm:px-6 py-4 grid grid-cols-1 lg:grid-cols-[300px_1fr] gap-4">
        <aside className="hidden lg:flex flex-col rounded-xl border border-gray-4 bg-gray-2 overflow-hidden">
          <div className="px-3 py-2 border-b border-gray-4 text-[12px] text-gray-8 uppercase tracking-wider">
            Conversations
          </div>
          <div className="overflow-y-auto divide-y divide-gray-4">
            {chats.map((item) => (
              <Link
                key={item.id}
                href={`/chat/${item.id}`}
                className={`block px-3 py-2.5 ${
                  item.id === chatId ? 'bg-gray-3' : 'hover:bg-gray-3/60'
                }`}
              >
                <div className="flex items-center gap-2">
                  <p className="text-[13px] text-gray-12 truncate font-medium">{item.title}</p>
                  <span className="ml-auto text-[10px] uppercase tracking-wide text-gray-7">{item.status}</span>
                </div>
                <p className="text-[11px] text-gray-7 mt-1">
                  {item.updated_at ? formatDistanceToNow(new Date(item.updated_at), { addSuffix: true }) : ''}
                </p>
              </Link>
            ))}
          </div>
        </aside>

        <section className="relative rounded-xl border border-gray-4 bg-gray-2 overflow-hidden flex flex-col min-h-0">
          <div className="px-3 sm:px-4 py-2.5 border-b border-gray-4 flex items-center gap-2">
            <button
              onClick={() => router.push('/chat')}
              className="lg:hidden h-7 w-7 flex items-center justify-center rounded-lg text-gray-8 hover:text-gray-11 hover:bg-gray-3"
            >
              <ArrowLeft size={15} weight="bold" />
            </button>
            <div className="min-w-0">
              <h1 className="text-[14px] font-medium text-gray-12 truncate">{chat.title}</h1>
              <p className="text-[11px] text-gray-7">
                {chat.repo ? `${chat.repo} · ` : ''}{chat.status}
              </p>
            </div>
            <span className={`ml-auto inline-flex items-center gap-1.5 text-[11px] font-medium ${
              streamStatus === 'open'
                ? 'text-green-400'
                : streamStatus === 'fallback'
                  ? 'text-amber-300'
                  : 'text-gray-7'
            }`}>
              {showTypingIndicator && <Lightning size={12} weight="fill" className="animate-pulse" />}
              {streamStatus === 'open' ? 'Streaming' : streamStatus === 'fallback' ? 'Polling' : 'Connecting'}
            </span>
          </div>

          <div
            ref={messagesRef}
            onScroll={updateScrollState}
            className="flex-1 min-h-0 overflow-y-auto px-3 sm:px-4 py-4 space-y-4"
          >
            {logsLoading && logs.length === 0 ? (
              <div className="flex items-center gap-2 text-[13px] text-gray-8">
                <CircleNotch size={14} weight="bold" className="animate-spin" />
                Loading messages...
              </div>
            ) : logs.length === 0 ? (
              <div className="h-full min-h-[220px] flex flex-col items-center justify-center text-gray-8 text-center px-6">
                <ChatCenteredText size={30} weight="thin" className="mb-2" />
                <p className="text-[14px] text-gray-11">No messages yet</p>
                <p className="text-[12px] text-gray-8 mt-1">
                  Send a message below to kick off this conversation.
                </p>
              </div>
            ) : (
              <TaskThreadView logs={logs} />
            )}

            {showTypingIndicator && (
              <div className="flex justify-start">
                <div className="max-w-[85%] rounded-2xl border border-gray-5 bg-gray-3/80 px-3 py-2">
                  <div className="flex items-center gap-1.5 text-[12px] text-gray-8">
                    <span className="h-1.5 w-1.5 rounded-full bg-gray-7 animate-pulse" />
                    <span className="h-1.5 w-1.5 rounded-full bg-gray-7 animate-pulse [animation-delay:120ms]" />
                    <span className="h-1.5 w-1.5 rounded-full bg-gray-7 animate-pulse [animation-delay:240ms]" />
                    <span className="ml-1">Karna is thinking...</span>
                  </div>
                </div>
              </div>
            )}
          </div>

          {showJumpToLatest && (
            <button
              type="button"
              onClick={() => scrollToBottom('smooth')}
              className="absolute bottom-24 right-4 z-10 rounded-full border border-gray-5 bg-gray-3/90 px-3 py-1.5 text-[11px] text-gray-9 hover:text-gray-11 hover:bg-gray-3"
            >
              Jump to latest
            </button>
          )}

          <form onSubmit={handleComposerSubmit} className="sticky bottom-0 border-t border-gray-4 bg-gray-2/95 backdrop-blur p-3 sm:p-4">
            <div className="flex items-end gap-2 rounded-xl border border-gray-4 bg-gray-1 px-2.5 py-2 transition-smooth focus-within:border-gray-6 focus-within:ring-2 focus-within:ring-sun-9/25">
              <textarea
                ref={composerRef}
                value={message}
                onChange={(e) => setMessage(e.target.value)}
                onKeyDown={handleComposerKeyDown}
                rows={1}
                placeholder="Send a follow-up..."
                className="flex-1 max-h-[200px] bg-transparent text-[14px] leading-6 text-gray-12 placeholder:text-gray-7 resize-none focus:outline-none py-1"
              />
              <Button
                type="submit"
                variant="primary"
                size="icon"
                className="shrink-0"
                aria-label="Send"
                disabled={commentMutation.isPending || !message.trim()}
              >
                {commentMutation.isPending ? (
                  <CircleNotch size={15} weight="bold" className="animate-spin" />
                ) : (
                  <PaperPlaneTilt size={15} weight="fill" />
                )}
              </Button>
            </div>
            <p className="mt-1.5 text-[11px] text-gray-7">
              Enter to send, Shift+Enter for newline.
            </p>
          </form>
        </section>
      </div>
    </div>
  );
}
