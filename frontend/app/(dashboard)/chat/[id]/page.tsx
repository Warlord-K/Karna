'use client';

import Link from 'next/link';
import { useParams, useRouter } from 'next/navigation';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useSession } from 'next-auth/react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { formatDistanceToNow } from 'date-fns';
import { ArrowLeft, ChatCenteredText, PaperPlaneTilt } from '@phosphor-icons/react';
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
  const logsEndRef = useRef<HTMLDivElement>(null);

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
    logsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [logs]);

  useEffect(() => {
    setStreamLogs([]);
    setStreamStatus('idle');
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
    source.onopen = () => setStreamStatus('open');
    source.onerror = () => {
      if (closed) return;
      closed = true;
      source.close();
      setStreamStatus('fallback');
    };

    return () => {
      closed = true;
      source.close();
    };
    // Reconnect when switching chats.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [chatId]);

  const commentMutation = useMutation({
    mutationFn: async () => {
      const trimmed = message.trim();
      if (!trimmed) throw new Error('Message is required');
      return postComment(chatId, trimmed);
    },
    onSuccess: async () => {
      setMessage('');
      await queryClient.invalidateQueries({ queryKey: ['tasks', 'logs', chatId] });
    },
    onError: () => {
      toast.error('Failed to send message');
    },
  });

  const handleSend = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!message.trim()) return;
    await commentMutation.mutateAsync();
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

  return (
    <div className="h-full overflow-hidden">
      <div className="h-full max-w-6xl mx-auto px-4 sm:px-6 py-4 grid grid-cols-1 lg:grid-cols-[280px_1fr] gap-4">
        <aside className="hidden lg:flex flex-col rounded-xl border border-gray-4 bg-gray-2 overflow-hidden">
          <div className="px-3 py-2 border-b border-gray-4 text-[12px] text-gray-8 uppercase tracking-wider">
            Conversations
          </div>
          <div className="overflow-y-auto">
            {chats.map((item) => (
              <Link
                key={item.id}
                href={`/chat/${item.id}`}
                className={`block px-3 py-2 border-b border-gray-4 last:border-b-0 ${
                  item.id === chatId ? 'bg-gray-3' : 'hover:bg-gray-3/60'
                }`}
              >
                <p className="text-[13px] text-gray-12 truncate">{item.title}</p>
                <p className="text-[11px] text-gray-7 mt-1">
                  {item.created_at ? formatDistanceToNow(new Date(item.created_at), { addSuffix: true }) : ''}
                </p>
              </Link>
            ))}
          </div>
        </aside>

        <section className="rounded-xl border border-gray-4 bg-gray-2 overflow-hidden flex flex-col min-h-0">
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
            <span className={`ml-auto text-[11px] font-medium ${
              streamStatus === 'open'
                ? 'text-green-400'
                : streamStatus === 'fallback'
                  ? 'text-amber-300'
                  : 'text-gray-7'
            }`}>
              {streamStatus === 'open' ? 'Streaming' : streamStatus === 'fallback' ? 'Polling' : 'Connecting'}
            </span>
          </div>

          <div className="flex-1 min-h-0 overflow-y-auto px-3 sm:px-4 py-4">
            {logsLoading && logs.length === 0 ? (
              <div className="text-[13px] text-gray-8">Loading messages...</div>
            ) : logs.length === 0 ? (
              <div className="text-[13px] text-gray-8">No messages yet.</div>
            ) : (
              <>
                <TaskThreadView logs={logs} />
                <div ref={logsEndRef} />
              </>
            )}
          </div>

          <form onSubmit={handleSend} className="border-t border-gray-4 p-3 sm:p-4">
            <div className="flex items-end gap-2">
              <textarea
                value={message}
                onChange={(e) => setMessage(e.target.value)}
                rows={2}
                placeholder="Send a follow-up..."
                className="flex-1 px-3 py-2 rounded-lg bg-gray-1 border border-gray-4 text-[14px] text-gray-12 placeholder:text-gray-7 focus:outline-none focus:border-gray-6 resize-none"
              />
              <button
                type="submit"
                disabled={commentMutation.isPending || !message.trim()}
                className="h-9 px-4 rounded-lg bg-sun-9 hover:bg-sun-10 text-gray-1 text-[13px] font-medium disabled:opacity-50 disabled:cursor-not-allowed inline-flex items-center gap-1.5"
              >
                <PaperPlaneTilt size={14} weight="bold" />
                Send
              </button>
            </div>
          </form>
        </section>
      </div>
    </div>
  );
}
