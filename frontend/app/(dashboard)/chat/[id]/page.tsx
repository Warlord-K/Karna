'use client';

import Link from 'next/link';
import { useParams, useRouter } from 'next/navigation';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useSession } from 'next-auth/react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { formatDistanceToNow } from 'date-fns';
import { ArrowLeft, ChatCenteredText, CircleNotch, PaperPlaneTilt, Plus } from '@phosphor-icons/react';
import toast from 'react-hot-toast';

import { useAuthDisabled } from '@/lib/auth-context';
import { chatStatusBadge, fetchChats, isChatWorking } from '@/lib/chats';
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
import { Badge } from '@/components/ui/badge';

const chatsKey = ['chats', 'list'] as const;

// SSE reconnect tuning. We retry with exponential backoff (1s, 2s, 4s, …) capped,
// and only declare a sustained outage (polling fallback + toast) after several
// consecutive failures so a single transient blip never downgrades streaming.
const SSE_FALLBACK_AFTER_ATTEMPTS = 3;
const SSE_BACKOFF_BASE_MS = 1000;
const SSE_BACKOFF_MAX_MS = 8000;
const SSE_FALLBACK_TOAST_ID = 'sse-fallback';

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
  // Holds the newest log so a reconnecting EventSource can resume from the right
  // cursor without forcing the stream effect to re-run on every new message.
  const latestLogRef = useRef<AgentLog | null>(null);
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

  useEffect(() => {
    latestLogRef.current = logs[logs.length - 1] ?? null;
  }, [logs]);

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
    latestLogRef.current = null;
    setShowJumpToLatest(false);
    toast.dismiss(SSE_FALLBACK_TOAST_ID);
  }, [chatId]);

  useEffect(() => {
    if (!chatId) return;
    if (typeof window === 'undefined' || typeof window.EventSource === 'undefined') {
      setStreamStatus('fallback');
      return;
    }

    // All mutable connection state lives in one object so the cleanup closure
    // (and StrictMode's double-invoke) can deterministically tear it down.
    const conn: {
      source: EventSource | null;
      timer: ReturnType<typeof setTimeout> | null;
      attempts: number;
      cancelled: boolean;
    } = { source: null, timer: null, attempts: 0, cancelled: false };

    const clearTimer = () => {
      if (conn.timer) {
        clearTimeout(conn.timer);
        conn.timer = null;
      }
    };

    const onLogData = (raw: string) => {
      try {
        const next = JSON.parse(raw) as AgentLog;
        if (!next?.id) return;
        setStreamLogs((current) => mergeTaskLogs(current, [next]));
      } catch {
        // Ignore malformed events and keep the stream alive.
      }
    };

    const connect = () => {
      if (conn.cancelled) return;
      clearTimer();

      const source = new EventSource(
        buildLogStreamUrl(chatId, encodeLogCursor(latestLogRef.current)),
      );
      conn.source = source;

      source.addEventListener('log', (event: MessageEvent) => onLogData(event.data));
      source.onmessage = (event) => onLogData(event.data);

      source.onopen = () => {
        if (conn.cancelled) return;
        conn.attempts = 0;
        setStreamStatus('open');
        // A reconnect succeeded — clear the outage toast and let the user know.
        if (fallbackNotifiedRef.current) {
          fallbackNotifiedRef.current = false;
          toast.dismiss(SSE_FALLBACK_TOAST_ID);
          toast.success('Live stream reconnected.', { id: SSE_FALLBACK_TOAST_ID });
        }
      };

      source.onerror = () => {
        if (conn.cancelled) return;
        // Take manual control of reconnection instead of EventSource's opaque retry.
        source.close();
        conn.source = null;
        conn.attempts += 1;

        if (conn.attempts >= SSE_FALLBACK_AFTER_ATTEMPTS) {
          // Sustained failure: surface polling fallback (the query below already
          // polls whenever streamStatus !== 'open') and notify once.
          setStreamStatus('fallback');
          if (!fallbackNotifiedRef.current) {
            fallbackNotifiedRef.current = true;
            toast.error('Live stream disconnected. Switched to polling.', {
              id: SSE_FALLBACK_TOAST_ID,
            });
          }
        } else {
          setStreamStatus('connecting');
        }

        // Exponential backoff, capped — 1s, 2s, 4s, 8s, 8s, …
        const delay = Math.min(
          SSE_BACKOFF_BASE_MS * 2 ** (conn.attempts - 1),
          SSE_BACKOFF_MAX_MS,
        );
        clearTimer();
        conn.timer = setTimeout(connect, delay);
      };
    };

    setStreamStatus('connecting');
    connect();

    return () => {
      conn.cancelled = true;
      clearTimer();
      conn.source?.close();
      conn.source = null;
    };
    // Reconnect when switching chats; cursor is read from latestLogRef at connect time.
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

  const isAgentWorking = isChatWorking(chat.status);
  const showTypingIndicator = isAgentWorking && (streamStatus === 'open' || streamStatus === 'connecting');
  const headerBadge = chatStatusBadge(chat.status);

  const streamIndicator =
    streamStatus === 'open'
      ? { label: 'Live', dot: 'bg-green-400', text: 'text-green-400' }
      : streamStatus === 'fallback'
        ? { label: 'Polling', dot: 'bg-amber-400', text: 'text-amber-300' }
        : { label: 'Connecting', dot: 'bg-gray-7 animate-pulse', text: 'text-gray-7' };

  return (
    <div className="h-full overflow-hidden">
      <div className="h-full px-4 sm:px-6 py-4 grid grid-cols-1 lg:grid-cols-[280px_1fr] gap-4">
        <aside className="hidden lg:flex flex-col rounded-xl border border-gray-4 bg-gray-2 overflow-hidden">
          <div className="px-3 py-2.5 border-b border-gray-4 flex items-center justify-between">
            <span className="text-[11px] font-medium text-gray-8 uppercase tracking-wider">Conversations</span>
            <Link
              href="/chat"
              className="h-6 w-6 flex items-center justify-center rounded-md text-gray-8 hover:text-gray-11 hover:bg-gray-3 transition-smooth focus-ring"
              aria-label="New chat"
            >
              <Plus size={14} weight="bold" />
            </Link>
          </div>
          <div className="overflow-y-auto p-1.5 space-y-0.5">
            {chats.length === 0 ? (
              <div className="px-3 py-6 text-center text-[12px] text-gray-7">No conversations yet.</div>
            ) : (
              chats.map((item) => {
                const itemBadge = chatStatusBadge(item.status);
                const active = item.id === chatId;
                return (
                  <Link
                    key={item.id}
                    href={`/chat/${item.id}`}
                    className={`block rounded-lg px-2.5 py-2 transition-smooth focus-ring ${
                      active ? 'bg-gray-3 border border-gray-5' : 'border border-transparent hover:bg-gray-3/60'
                    }`}
                  >
                    <div className="flex items-center gap-2">
                      <p className={`text-[13px] truncate ${active ? 'text-gray-12 font-medium' : 'text-gray-11'}`}>
                        {item.title}
                      </p>
                      <Badge tone={itemBadge.tone} className="ml-auto flex-shrink-0">
                        {itemBadge.label}
                      </Badge>
                    </div>
                    <p className="text-[11px] text-gray-7 mt-1 truncate">
                      {item.repo ? `${item.repo} · ` : ''}
                      {item.updated_at ? formatDistanceToNow(new Date(item.updated_at), { addSuffix: true }) : ''}
                    </p>
                  </Link>
                );
              })
            )}
          </div>
        </aside>

        <section className="relative rounded-xl border border-gray-4 bg-gray-2 overflow-hidden flex flex-col min-h-0">
          <div className="px-3 sm:px-4 py-2.5 border-b border-gray-4 flex items-center gap-2.5 bg-gray-2/80 backdrop-blur">
            <button
              onClick={() => router.push('/chat')}
              className="lg:hidden h-7 w-7 flex items-center justify-center rounded-lg text-gray-8 hover:text-gray-11 hover:bg-gray-3 transition-smooth focus-ring shrink-0"
              aria-label="Back to chats"
            >
              <ArrowLeft size={15} weight="bold" />
            </button>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2 min-w-0">
                <h1 className="text-[14px] font-medium text-gray-12 truncate">{chat.title}</h1>
                <Badge tone={headerBadge.tone} className="flex-shrink-0">{headerBadge.label}</Badge>
              </div>
              {chat.repo && (
                <p className="text-[11px] text-gray-7 truncate mt-0.5 font-mono">{chat.repo}</p>
              )}
            </div>
            <span
              className={`ml-auto inline-flex items-center gap-1.5 text-[11px] font-medium shrink-0 ${streamIndicator.text}`}
              title={`Updates: ${streamIndicator.label}`}
            >
              <span className={`h-1.5 w-1.5 rounded-full ${streamIndicator.dot}`} />
              <span className="hidden sm:inline">{streamIndicator.label}</span>
            </span>
          </div>

          <div
            ref={messagesRef}
            onScroll={updateScrollState}
            className="flex-1 min-h-0 overflow-y-auto px-3 sm:px-6 lg:px-8 py-5"
          >
            <div className="w-full">
              {logsLoading && logs.length === 0 ? (
                <div className="flex items-center gap-2 text-[13px] text-gray-8">
                  <CircleNotch size={14} weight="bold" className="animate-spin" />
                  Loading messages...
                </div>
              ) : logs.length === 0 ? (
                <div className="min-h-[260px] flex flex-col items-center justify-center text-center px-6">
                  <div className="h-11 w-11 rounded-full border border-gray-5 bg-gray-3 flex items-center justify-center mb-3 text-sun-9">
                    <ChatCenteredText size={20} weight="bold" />
                  </div>
                  <p className="text-[14px] text-gray-11 font-medium">No messages yet</p>
                  <p className="text-[12px] text-gray-8 mt-1 max-w-[280px]">
                    Send a message below to kick off this conversation with Karna.
                  </p>
                </div>
              ) : (
                <TaskThreadView logs={logs} />
              )}

              {showTypingIndicator && (
                <div className="flex justify-start mt-3">
                  <div className="inline-flex items-center gap-2 rounded-2xl border border-gray-5 bg-gray-3/70 px-3 py-2">
                    <span className="flex items-center gap-1">
                      <span className="h-1.5 w-1.5 rounded-full bg-sun-9 animate-pulse" />
                      <span className="h-1.5 w-1.5 rounded-full bg-sun-9 animate-pulse [animation-delay:140ms]" />
                      <span className="h-1.5 w-1.5 rounded-full bg-sun-9 animate-pulse [animation-delay:280ms]" />
                    </span>
                    <span className="text-[12px] text-gray-9">Karna is working…</span>
                  </div>
                </div>
              )}
            </div>
          </div>

          {showJumpToLatest && (
            <button
              type="button"
              onClick={() => scrollToBottom('smooth')}
              className="absolute bottom-28 left-1/2 -translate-x-1/2 z-10 inline-flex items-center gap-1.5 rounded-full border border-gray-5 bg-gray-3/95 backdrop-blur px-3 py-1.5 text-[11px] text-gray-9 shadow-sm transition-smooth press-scale focus-ring hover:text-gray-11 hover:bg-gray-3 animate-fade-in"
            >
              <ArrowLeft size={11} weight="bold" className="-rotate-90" />
              Jump to latest
            </button>
          )}

          <form onSubmit={handleComposerSubmit} className="sticky bottom-0 border-t border-gray-4 bg-gray-2/95 backdrop-blur p-3 sm:p-4 sm:px-6 lg:px-8">
            <div className="w-full">
              <div className="flex items-end gap-2 rounded-xl border border-gray-4 bg-gray-1 px-2.5 py-2 transition-smooth focus-within:border-gray-6 focus-within:ring-2 focus-within:ring-sun-9/25">
                <textarea
                  ref={composerRef}
                  value={message}
                  onChange={(e) => setMessage(e.target.value)}
                  onKeyDown={handleComposerKeyDown}
                  rows={1}
                  placeholder="Send a follow-up…"
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
                <kbd className="font-sans text-gray-8">Enter</kbd> to send · <kbd className="font-sans text-gray-8">Shift+Enter</kbd> for newline
              </p>
            </div>
          </form>
        </section>
      </div>
    </div>
  );
}
