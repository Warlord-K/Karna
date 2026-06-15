'use client';

import { useMemo } from 'react';
import Link from 'next/link';
import {
  ArrowSquareOut,
  CaretRight,
  ChatText,
  FileText,
  Robot,
  Stack,
  Terminal,
  WarningCircle,
} from '@phosphor-icons/react';
import { format } from 'date-fns';
import { MarkdownContent } from '@/components/agent/markdown-content';
import { Badge } from '@/components/ui/badge';
import { Card } from '@/components/ui/card';
import {
  taskStatusBadge,
  type AgentLog,
  type AgentTaskStatus,
} from '@/lib/agent-tasks';

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

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

/** Tool output can be huge — keep the DOM light and the panel scrollable. */
function truncateOutput(text: string, max = 16000): string {
  if (text.length <= max) return text;
  return `${text.slice(0, max)}\n… (${text.length - max} more characters truncated)`;
}

function humanize(value: string): string {
  return value.replace(/[_-]+/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
}

function isUrl(value: string | undefined): value is string {
  return !!value && /^https?:\/\//i.test(value);
}

function isSystemEvent(log: AgentLog): boolean {
  if (log.log_type === 'command') return true;
  if (log.log_type !== 'info') return false;
  const message = log.message?.trim() ?? '';
  if (!message || looksLikeCodeOrDiff(message)) return false;
  return message.length <= 120;
}

const PHASE_LABELS: Record<string, string> = {
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
  return PHASE_LABELS[phase] ?? humanize(phase);
}

// ---------------------------------------------------------------------------
// Metadata contract parsing — degrade gracefully when fields are absent.
// metadata is Record<string, unknown> | null, so every read is defensive.
// ---------------------------------------------------------------------------

function asString(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() ? value : undefined;
}

function asNumber(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
}

interface ToolMeta {
  tool?: string;
  input?: string;
  output?: string;
}

function readToolMeta(log: AgentLog): ToolMeta {
  const meta = log.metadata ?? {};
  return {
    tool: asString(meta.tool),
    input: asString(meta.input),
    output: asString(meta.output),
  };
}

interface TaskCardMeta {
  task_id: string;
  title: string;
  status: AgentTaskStatus;
  task_number?: number;
  kind?: string;
}

function readTaskCard(log: AgentLog): TaskCardMeta | null {
  if (log.log_type !== 'info' || !log.metadata) return null;
  if (log.metadata.card !== 'task') return null;
  const task_id = asString(log.metadata.task_id);
  const title = asString(log.metadata.title);
  const status = asString(log.metadata.status);
  if (!task_id || !title || !status) return null;
  return {
    task_id,
    title,
    status: status as AgentTaskStatus,
    task_number: asNumber(log.metadata.task_number),
    kind: asString(log.metadata.kind),
  };
}

interface ArtifactCardMeta {
  output_target?: string;
  output_ref?: string;
}

function readArtifactCard(log: AgentLog): ArtifactCardMeta | null {
  if (log.log_type !== 'output' || !log.metadata) return null;
  if (log.metadata.card !== 'artifact') return null;
  return {
    output_target: asString(log.metadata.output_target),
    output_ref: asString(log.metadata.output_ref),
  };
}

// ---------------------------------------------------------------------------
// Timeline model
// ---------------------------------------------------------------------------

type BubbleRole = 'user' | 'assistant' | 'error';

type TimelineItem =
  | { kind: 'phase'; id: string; phase: string }
  | { kind: 'bubble'; id: string; role: BubbleRole; logs: AgentLog[] }
  | { kind: 'tool'; id: string; log: AgentLog; meta: ToolMeta }
  | { kind: 'system'; id: string; log: AgentLog }
  | { kind: 'task-card'; id: string; log: AgentLog; card: TaskCardMeta }
  | { kind: 'artifact-card'; id: string; log: AgentLog; card: ArtifactCardMeta };

export function TaskThreadView({ logs }: { logs: AgentLog[] }) {
  // Only show phase dividers when the thread actually spans multiple phases
  // (otherwise a single redundant divider would just add noise to chat).
  const showPhases = useMemo(() => {
    const phases = new Set(logs.map((log) => log.phase).filter(Boolean));
    return phases.size > 1;
  }, [logs]);

  const timeline = useMemo<TimelineItem[]>(() => {
    const items: TimelineItem[] = [];
    let lastPhase: string | null = null;

    for (const log of logs) {
      if (showPhases && log.phase && log.phase !== lastPhase) {
        items.push({ kind: 'phase', id: `phase-${log.id}`, phase: log.phase });
        lastPhase = log.phase;
      }

      const taskCard = readTaskCard(log);
      if (taskCard) {
        items.push({ kind: 'task-card', id: log.id, log, card: taskCard });
        continue;
      }

      const artifactCard = readArtifactCard(log);
      if (artifactCard) {
        items.push({ kind: 'artifact-card', id: log.id, log, card: artifactCard });
        continue;
      }

      if (log.log_type === 'tool') {
        items.push({ kind: 'tool', id: log.id, log, meta: readToolMeta(log) });
        continue;
      }

      if (isSystemEvent(log)) {
        items.push({ kind: 'system', id: log.id, log });
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
        items.push({ kind: 'bubble', id: log.id, role, logs: [log] });
      }
    }

    return items;
  }, [logs, showPhases]);

  return (
    <div className="space-y-4">
      {timeline.map((item) => {
        switch (item.kind) {
          case 'phase':
            return <PhaseDivider key={item.id} phase={item.phase} />;
          case 'task-card':
            return <TaskCard key={item.id} card={item.card} />;
          case 'artifact-card':
            return <ArtifactCard key={item.id} card={item.card} log={item.log} />;
          case 'tool':
            return <ToolChip key={item.id} log={item.log} meta={item.meta} />;
          case 'system':
            return <SystemLine key={item.id} log={item.log} />;
          case 'bubble':
          default:
            return item.role === 'assistant'
              ? <AssistantTurn key={item.id} logs={item.logs} />
              : <PersonBubble key={item.id} role={item.role} logs={item.logs} />;
        }
      })}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Renderers
// ---------------------------------------------------------------------------

function PhaseDivider({ phase }: { phase: string }) {
  return (
    <div className="flex items-center gap-2 pt-1">
      <span className="text-[10px] font-semibold uppercase tracking-wider text-gray-7">
        {phaseLabel(phase)}
      </span>
      <span className="h-px flex-1 bg-gray-4/60" />
    </div>
  );
}

/** Assistant / agent response — the focal content of the thread. */
function AssistantTurn({ logs }: { logs: AgentLog[] }) {
  return (
    <div className="flex w-full justify-start">
      <div className="flex w-full gap-3">
        <div className="mt-0.5 flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-full border border-sun-8/50 bg-sun-9/15 text-sun-10">
          <Robot size={14} weight="bold" />
        </div>
        <div className="min-w-0 flex-1 space-y-1.5 max-w-3xl">
          <p className="text-[11px] font-semibold text-sun-10/90">Karna</p>
          {logs.map((log) => {
            const time = formatMessageTime(log.created_at);
            return (
              <div key={log.id}>
                <MarkdownContent
                  content={formatLogMessage(log)}
                  className="text-[15px] [&_li]:text-[15px] [&_li]:text-gray-12 [&_p]:mb-2.5 [&_p:last-child]:mb-0 [&_p]:text-[15px] [&_p]:leading-7 [&_p]:text-gray-12"
                />
                {time && <p className="mt-1 text-[10px] text-gray-6">{time}</p>}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

/** User comments (right-aligned bubble) and error turns (red bubble). */
function PersonBubble({ role, logs }: { role: 'user' | 'error'; logs: AgentLog[] }) {
  const isUser = role === 'user';
  const isError = role === 'error';
  const roleLabel = isUser ? 'You' : 'Error';

  return (
    <div className={`flex w-full ${isUser ? 'justify-end' : 'justify-start'}`}>
      <div className={`flex gap-2.5 ${
        isUser ? 'max-w-[88%] flex-row-reverse sm:max-w-[680px]' : 'w-full flex-row'
      }`}>
        <div className={`mt-0.5 flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-full border ${
          isError
            ? 'border-red-500/40 bg-red-500/15 text-red-300'
            : 'border-gray-5 bg-gray-3 text-gray-9'
        }`}>
          {isError ? <WarningCircle size={14} weight="fill" /> : <ChatText size={14} weight="bold" />}
        </div>

        <div className={`min-w-0 flex-1 space-y-1 ${isUser ? 'items-end text-right' : 'items-start text-left max-w-3xl'}`}>
          <p className={`text-[11px] font-medium ${isError ? 'text-red-300' : 'text-gray-8'}`}>
            {roleLabel}
          </p>
          {logs.map((log) => {
            const time = formatMessageTime(log.created_at);
            return (
              <div
                key={log.id}
                className={`inline-block max-w-full rounded-2xl border px-3.5 py-2.5 text-left ${
                  isUser
                    ? 'border-sun-8 bg-sun-9 text-gray-1'
                    : 'border-red-500/40 bg-red-500/12 text-red-200'
                }`}
              >
                {isUser ? (
                  <p className="whitespace-pre-wrap break-words text-[14px] leading-6">{log.message}</p>
                ) : (
                  <MarkdownContent
                    content={formatLogMessage(log)}
                    className="text-[14px] [&_p]:mb-2 [&_p:last-child]:mb-0 [&_p]:leading-relaxed [&_p]:text-red-200"
                  />
                )}
                {time && (
                  <p className={`mt-1 text-[10px] ${isUser ? 'text-gray-1/70' : 'text-red-200/80'}`}>
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
}

/** Compact, muted, collapsible tool chip. Collapsed: `tool: input`.
 *  Expanded: the real command output in a scrollable monospace block. */
function ToolChip({ log, meta }: { log: AgentLog; meta: ToolMeta }) {
  const time = formatMessageTime(log.created_at);
  const label = meta.tool ?? 'tool';
  const input = meta.input ?? summarizeChipMessage(log.message);
  const output = meta.output;

  return (
    <div className="flex justify-start">
      <details className="group max-w-full [&_summary::-webkit-details-marker]:hidden">
        <summary className="inline-flex max-w-full cursor-pointer list-none items-center gap-1.5 rounded-md border border-gray-4 bg-gray-3/50 px-2 py-1 text-[11px] text-gray-8 transition-smooth focus-ring hover:bg-gray-3">
          <CaretRight size={11} weight="bold" className="flex-shrink-0 text-gray-7 transition-transform group-open:rotate-90" />
          <Terminal size={11} weight="bold" className="flex-shrink-0 text-gray-7" />
          <span className="font-mono font-medium text-gray-9">{label}</span>
          {input && (
            <span className="max-w-[320px] truncate font-mono text-gray-7">{input}</span>
          )}
          {time && <span className="flex-shrink-0 text-gray-7">{time}</span>}
        </summary>
        <div className="mt-1.5 max-w-3xl">
          {output ? (
            <pre className="max-h-[360px] overflow-auto whitespace-pre-wrap break-words rounded-lg border border-gray-4 bg-gray-2 px-3 py-2 font-mono text-[11.5px] leading-[1.6] text-gray-10">
              {truncateOutput(output)}
            </pre>
          ) : (
            <div className="rounded-lg border border-gray-4 bg-gray-2 px-3 py-2">
              <MarkdownContent
                content={formatLogMessage(log)}
                className="text-[12.5px] [&_p]:mb-1.5 [&_p:last-child]:mb-0"
              />
            </div>
          )}
        </div>
      </details>
    </div>
  );
}

/** System / info events — muted secondary lines. Long ones collapse. */
function SystemLine({ log }: { log: AgentLog }) {
  const time = formatMessageTime(log.created_at);
  const hasDetails = looksLikeCodeOrDiff(log.message) || log.message.length > 140;

  if (!hasDetails) {
    return (
      <div className="flex justify-start pl-1">
        <div className="inline-flex max-w-full items-center gap-1.5 text-[11px] text-gray-7">
          <span className="h-1 w-1 flex-shrink-0 rounded-full bg-gray-6" />
          <span className="max-w-[460px] truncate">{log.message}</span>
          {time && <span className="flex-shrink-0 text-gray-6">{time}</span>}
        </div>
      </div>
    );
  }

  return (
    <div className="flex justify-start">
      <details className="group max-w-full [&_summary::-webkit-details-marker]:hidden">
        <summary className="inline-flex max-w-full cursor-pointer list-none items-center gap-1.5 rounded-md px-1 py-0.5 text-[11px] text-gray-7 transition-smooth focus-ring hover:text-gray-9">
          <CaretRight size={11} weight="bold" className="flex-shrink-0 transition-transform group-open:rotate-90" />
          <span className="max-w-[420px] truncate">{summarizeChipMessage(log.message)}</span>
          {time && <span className="flex-shrink-0 text-gray-6">{time}</span>}
        </summary>
        <div className="mt-1.5 max-w-3xl rounded-lg border border-gray-4 bg-gray-2 px-3 py-2">
          <MarkdownContent
            content={formatLogMessage(log)}
            className="text-[12.5px] [&_p]:mb-1.5 [&_p:last-child]:mb-0"
          />
        </div>
      </details>
    </div>
  );
}

/** A spawned task surfaced as a clickable card linking to its detail page. */
function TaskCard({ card }: { card: TaskCardMeta }) {
  const badge = taskStatusBadge(card.status);
  const ref = card.task_number != null ? `#${card.task_number}` : null;

  return (
    <div className="flex w-full justify-start">
      <div className="w-full max-w-3xl">
        <p className="mb-1 flex items-center gap-1.5 pl-1 text-[11px] text-gray-7">
          <Stack size={12} weight="bold" className="text-sun-10" />
          Spawned task
        </p>
        <Link href={`/tasks/${card.task_id}`} className="block focus-ring rounded-xl">
          <Card interactive className="px-3.5 py-3">
            <div className="flex items-center gap-3">
              <div className="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-lg border border-gray-5 bg-gray-3 text-sun-10">
                <Stack size={15} weight="bold" />
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  {ref && <span className="font-mono text-[11px] text-gray-7">{ref}</span>}
                  {card.kind && (
                    <span className="font-mono text-[11px] uppercase tracking-wider text-gray-7">{card.kind}</span>
                  )}
                  <Badge tone={badge.tone}>{badge.label}</Badge>
                </div>
                <p className="mt-0.5 truncate text-[14px] font-medium text-gray-12">{card.title}</p>
              </div>
              <CaretRight size={14} weight="bold" className="flex-shrink-0 text-gray-7" />
            </div>
          </Card>
        </Link>
      </div>
    </div>
  );
}

/** An artifact the orchestrator produced — rendered markdown with a header
 *  and, when the ref is a URL, a link to open the published output. */
function ArtifactCard({ card, log }: { card: ArtifactCardMeta; log: AgentLog }) {
  const heading = card.output_target ? humanize(card.output_target) : 'Artifact';

  return (
    <div className="flex w-full justify-start">
      <Card className="w-full max-w-3xl overflow-hidden">
        <div className="flex items-center gap-2 border-b border-gray-4 bg-gray-3/40 px-3.5 py-2">
          <FileText size={14} weight="bold" className="flex-shrink-0 text-sun-10" />
          <span className="text-[12px] font-medium text-gray-11">{heading}</span>
          {isUrl(card.output_ref) && (
            <a
              href={card.output_ref}
              target="_blank"
              rel="noopener noreferrer"
              className="ml-auto inline-flex items-center gap-1 text-[12px] text-sun-10 transition-smooth focus-ring hover:text-sun-9"
            >
              Open <ArrowSquareOut size={12} weight="bold" />
            </a>
          )}
        </div>
        <div className="max-h-[420px] overflow-y-auto px-3.5 py-3">
          <MarkdownContent content={log.message?.trim() || '_No content_'} />
        </div>
      </Card>
    </div>
  );
}
