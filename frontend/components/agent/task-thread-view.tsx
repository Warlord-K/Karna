'use client';

import { useMemo } from 'react';
import { CaretDown, ChatText } from '@phosphor-icons/react';
import { format } from 'date-fns';
import { MarkdownContent } from '@/components/agent/markdown-content';
import type { AgentLog } from '@/lib/agent-tasks';

const phaseLabels: Record<string, string> = {
  plan: 'Plan',
  planning: 'Plan',
  implement: 'Implement',
  in_progress: 'Implement',
  self_review: 'Self Review',
  review: 'Review',
  feedback: 'Feedback',
  user: 'Feedback',
};

function phaseLabel(phase: string): string {
  if (phaseLabels[phase]) return phaseLabels[phase];
  return phase.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
}

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

function ThreadMessage({ log }: { log: AgentLog }) {
  const time = format(new Date(log.created_at), 'HH:mm:ss');

  if (log.log_type === 'comment') {
    return (
      <div className="ml-auto max-w-[90%] rounded-lg bg-sun-3 border border-sun-5 px-3 py-2">
        <div className="flex items-center gap-2 mb-1">
          <ChatText size={13} weight="bold" className="text-sun-9 flex-shrink-0" />
          <span className="text-[11px] text-sun-9 font-medium">You</span>
          <span className="text-[11px] text-gray-7 ml-auto">{time}</span>
        </div>
        <p className="text-[13px] text-gray-12 whitespace-pre-wrap break-words font-sans">{log.message}</p>
      </div>
    );
  }

  if (log.log_type === 'tool') {
    return (
      <details className="rounded-lg border border-gray-4 bg-gray-3/60 px-3 py-2">
        <summary className="list-none cursor-pointer flex items-center gap-2 text-[12px] text-sun-9">
          <CaretDown size={13} weight="bold" className="text-sun-9" />
          <span className="font-medium">Tool Call</span>
          <span className="text-gray-8 truncate">{log.message}</span>
          <span className="text-[11px] text-gray-7 ml-auto">{time}</span>
        </summary>
        <div className="mt-2 pt-2 border-t border-gray-4">
          <p className="text-[12px] text-gray-10 break-all whitespace-pre-wrap font-mono">{log.message}</p>
          {log.metadata && (
            <pre className="mt-2 p-2 rounded bg-gray-2 border border-gray-4 text-[11px] text-gray-9 overflow-x-auto">
              {JSON.stringify(log.metadata, null, 2)}
            </pre>
          )}
        </div>
      </details>
    );
  }

  const isError = log.log_type === 'error';
  return (
    <div className={`rounded-lg border px-3 py-2 ${
      isError ? 'border-red-500/30 bg-red-500/10' : 'border-gray-4 bg-gray-3/40'
    }`}>
      <div className="flex items-center gap-2 mb-1">
        <span className={`text-[11px] font-medium ${isError ? 'text-red-300' : 'text-gray-8'}`}>
          {isError ? 'Error' : 'Assistant'}
        </span>
        <span className="text-[11px] text-gray-7 ml-auto">{time}</span>
      </div>
      <MarkdownContent content={formatLogMessage(log)} className="text-[13px]" />
    </div>
  );
}

export function TaskThreadView({ logs }: { logs: AgentLog[] }) {
  const sections = useMemo(() => {
    const grouped: { phase: string; logs: AgentLog[] }[] = [];
    for (const log of logs) {
      const prev = grouped[grouped.length - 1];
      if (prev && prev.phase === log.phase) {
        prev.logs.push(log);
      } else {
        grouped.push({ phase: log.phase, logs: [log] });
      }
    }
    return grouped;
  }, [logs]);

  return (
    <div className="space-y-4">
      {sections.map((section, idx) => (
        <div key={`${section.phase}-${idx}`} className="space-y-2">
          <div className="text-[11px] uppercase tracking-wider text-gray-7 font-medium">
            {phaseLabel(section.phase)}
          </div>
          <div className="space-y-2">
            {section.logs.map((log) => (
              <ThreadMessage key={log.id} log={log} />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
