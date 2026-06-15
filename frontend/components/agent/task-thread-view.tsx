'use client';

import { useMemo } from 'react';
import { CaretDown, ChatText, Robot, Terminal, WarningCircle } from '@phosphor-icons/react';
import { format } from 'date-fns';
import { MarkdownContent } from '@/components/agent/markdown-content';
import type { AgentLog } from '@/lib/agent-tasks';

function looksLikeCodeOrDiff(message: string): boolean {
  const trimmed = message.trim();
  if (!trimmed) return false;
  return (
    trimmed.startsWith('diff --git')
    || trimmed.startsWith('@@ ')
    || trimmed.includes('\n@@ ')
    || trimmed.startsWith('```')
  );
}

function formatLogMessage(log: AgentLog): string {
  const message = log.message?.trim() || '_No message_';
  if (!looksLikeCodeOrDiff(message)) return message;
  if (message.startsWith('```')) return message;
  return `\`\`\`diff\n${message}\n\`\`\``;
}

function formatMessageTime(isoString: string): string {
  const parsed = new Date(isoString);
  if (Number.isNaN(parsed.getTime())) return '';
  return format(parsed, 'HH:mm');
}

function summarizeChipMessage(message: string): string {
  const flattened = message.replace(/\s+/g, ' ').trim();
  if (!flattened) return 'No details';
  return flattened.length > 90 ? `${flattened.slice(0, 90).trim()}...` : flattened;
}

function isSystemEvent(log: AgentLog): boolean {
  if (log.log_type === 'command') return true;
  if (log.log_type !== 'info') return false;
  const message = log.message?.trim() ?? '';
  if (!message || looksLikeCodeOrDiff(message)) return false;
  return message.length <= 120;
}

type BubbleRole = 'user' | 'assistant' | 'error';

type TimelineItem =
  | { kind: 'bubble'; role: BubbleRole; logs: AgentLog[] }
  | { kind: 'tool'; log: AgentLog }
  | { kind: 'system'; log: AgentLog };

export function TaskThreadView({ logs }: { logs: AgentLog[] }) {
  const timeline = useMemo<TimelineItem[]>(() => {
    const items: TimelineItem[] = [];

    for (const log of logs) {
      if (log.log_type === 'tool') {
        items.push({ kind: 'tool', log });
        continue;
      }

      if (isSystemEvent(log)) {
        items.push({ kind: 'system', log });
        continue;
      }

      const role: BubbleRole = log.log_type === 'comment'
        ? 'user'
        : log.log_type === 'error'
          ? 'error'
          : 'assistant';

      const prev = items[items.length - 1];
      if (prev?.kind === 'bubble' && prev.role === role) {
        prev.logs.push(log);
      } else {
        items.push({ kind: 'bubble', role, logs: [log] });
      }
    }

    return items;
  }, [logs]);

  return (
    <div className="space-y-3">
      {timeline.map((item, index) => {
        if (item.kind === 'tool') {
          const time = formatMessageTime(item.log.created_at);
          return (
            <div key={item.log.id} className="flex justify-start">
              <details className="max-w-full [&_summary::-webkit-details-marker]:hidden">
                <summary className="list-none inline-flex cursor-pointer items-center gap-1.5 rounded-full border border-gray-5 bg-gray-3/70 px-2.5 py-1 text-[11px] text-gray-8">
                  <CaretDown size={12} weight="bold" className="text-gray-8" />
                  <Terminal size={12} weight="bold" className="text-sun-9" />
                  <span className="font-medium text-gray-9">Ran tool</span>
                  <span className="max-w-[280px] truncate text-gray-8">{summarizeChipMessage(item.log.message)}</span>
                  {time && <span className="text-gray-7">{time}</span>}
                </summary>
                <div className="mt-2 max-w-3xl rounded-lg border border-gray-4 bg-gray-3/60 px-3 py-2">
                  <MarkdownContent
                    content={formatLogMessage(item.log)}
                    className="text-[13px] [&_p]:mb-2 [&_p:last-child]:mb-0"
                  />
                  {item.log.metadata && (
                    <pre className="mt-2 rounded-md border border-gray-4 bg-gray-2 p-2 text-[11px] text-gray-9 overflow-x-auto">
                      {JSON.stringify(item.log.metadata, null, 2)}
                    </pre>
                  )}
                </div>
              </details>
            </div>
          );
        }

        if (item.kind === 'system') {
          const time = formatMessageTime(item.log.created_at);
          const hasDetails = looksLikeCodeOrDiff(item.log.message) || item.log.message.length > 140;

          if (!hasDetails) {
            return (
              <div key={item.log.id} className="flex justify-center">
                <div className="inline-flex max-w-full items-center gap-1.5 rounded-full border border-gray-5 bg-gray-3/50 px-2.5 py-1 text-[11px] text-gray-8">
                  <Terminal size={11} weight="bold" className="text-gray-8" />
                  <span className="truncate max-w-[360px]">{item.log.message}</span>
                  {time && <span className="text-gray-7">{time}</span>}
                </div>
              </div>
            );
          }

          return (
            <div key={item.log.id} className="flex justify-center">
              <details className="max-w-full [&_summary::-webkit-details-marker]:hidden">
                <summary className="list-none inline-flex cursor-pointer items-center gap-1.5 rounded-full border border-gray-5 bg-gray-3/50 px-2.5 py-1 text-[11px] text-gray-8">
                  <CaretDown size={12} weight="bold" className="text-gray-8" />
                  <Terminal size={11} weight="bold" className="text-gray-8" />
                  <span className="max-w-[360px] truncate">{summarizeChipMessage(item.log.message)}</span>
                  {time && <span className="text-gray-7">{time}</span>}
                </summary>
                <div className="mt-2 max-w-3xl rounded-lg border border-gray-4 bg-gray-3/50 px-3 py-2">
                  <MarkdownContent
                    content={formatLogMessage(item.log)}
                    className="text-[13px] [&_p]:mb-2 [&_p:last-child]:mb-0"
                  />
                </div>
              </details>
            </div>
          );
        }

        const isUser = item.role === 'user';
        const isError = item.role === 'error';
        const roleLabel = isUser ? 'You' : isError ? 'Error' : 'Karna';

        return (
          <div key={`${item.role}-${index}`} className={`flex ${isUser ? 'justify-end' : 'justify-start'}`}>
            <div className={`flex max-w-[92%] gap-2 ${isUser ? 'flex-row-reverse' : 'flex-row'}`}>
              {!isUser && (
                <div className={`mt-1 flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-full border ${
                  isError
                    ? 'border-red-500/40 bg-red-500/15 text-red-300'
                    : 'border-gray-5 bg-gray-3 text-gray-9'
                }`}>
                  {isError ? <WarningCircle size={13} weight="fill" /> : <Robot size={13} weight="bold" />}
                </div>
              )}

              {isUser && (
                <div className="mt-1 flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-full border border-sun-8 bg-sun-9/20 text-sun-10">
                  <ChatText size={13} weight="bold" />
                </div>
              )}

              <div className={`space-y-1 ${isUser ? 'items-end text-right' : 'items-start text-left'}`}>
                <p className={`text-[11px] ${isUser ? 'text-sun-10/80' : isError ? 'text-red-300' : 'text-gray-7'}`}>
                  {roleLabel}
                </p>
                {item.logs.map((log) => {
                  const time = formatMessageTime(log.created_at);
                  return (
                    <div
                      key={log.id}
                      className={`rounded-2xl border px-3 py-2 ${
                        isUser
                          ? 'border-sun-8 bg-sun-9 text-gray-1'
                          : isError
                            ? 'border-red-500/40 bg-red-500/12 text-red-200'
                            : 'border-gray-5 bg-gray-3/80 text-gray-11'
                      }`}
                    >
                      {isUser ? (
                        <p className="whitespace-pre-wrap break-words text-[13px]">{log.message}</p>
                      ) : (
                        <MarkdownContent
                          content={formatLogMessage(log)}
                          className={`text-[13px] [&_p]:mb-2 [&_p:last-child]:mb-0 ${
                            isError ? '[&_p]:text-red-200' : '[&_p]:text-gray-10'
                          }`}
                        />
                      )}
                      {time && (
                        <p className={`mt-1 text-[10px] ${
                          isUser ? 'text-gray-1/70' : isError ? 'text-red-200/80' : 'text-gray-7'
                        }`}>
                          {time}
                        </p>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}
