'use client';

import { AgentTask, PRIORITY_COLORS, getTaskLabel, getTaskTitle } from '@/lib/agent-tasks';
import { AgentProfile } from '@/lib/agents';
import { GitPullRequest, WarningCircle, Lightning, Stack, Clock, User } from '@phosphor-icons/react';
import { formatDistanceToNow } from 'date-fns';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { cn } from '@/lib/utils';
import { Badge } from '@/components/ui/badge';

const statusConfig: Record<string, { label: string; color: string; pulse?: boolean }> = {
  todo:         { label: 'Todo',       color: '#a09e97' },
  planning:     { label: 'Planning',   color: '#e5b847', pulse: true },
  plan_review:  { label: 'Plan ready', color: '#e5b847' },
  in_progress:  { label: 'Working',    color: '#e5b847', pulse: true },
  review:       { label: 'In review',  color: '#60a5a0' },
  done:         { label: 'Done',       color: '#6ab070' },
  failed:       { label: 'Failed',     color: '#d4583a' },
  cancelled:    { label: 'Cancelled',  color: '#82807a' },
};

interface TaskCardProps {
  task: AgentTask;
  onClick: () => void;
  /**
   * Display label for the task creator. Set only when the workspace is shared
   * AND the task wasn't created by the current viewer (so people know whose
   * task they're touching).
   */
  creatorLabel?: string | null;
  /** Agent profile this task is assigned to (resolved from task.assigned_agent_id). */
  assignedAgent?: AgentProfile | null;
  /** Rendered inside the drag overlay — disables sortable wiring and entrance anim. */
  overlay?: boolean;
}

export function TaskCard({ task, onClick, creatorLabel, assignedAgent, overlay = false }: TaskCardProps) {
  const sortable = useSortable({ id: task.id, disabled: overlay });
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = sortable;

  const style = overlay
    ? undefined
    : {
        transform: CSS.Transform.toString(transform),
        transition,
        opacity: isDragging ? 0.35 : 1,
      };

  const status = statusConfig[task.status];
  const repoName = task.repo ? (task.repo.split('/').pop() || task.repo) : null;
  const hasSubtasks = (task.subtask_count ?? 0) > 0;

  return (
    <div
      ref={overlay ? undefined : setNodeRef}
      style={style}
      {...(overlay ? {} : attributes)}
      {...(overlay ? {} : listeners)}
      onClick={overlay ? undefined : onClick}
      className={cn(
        'group px-3 py-2.5 rounded-lg border transition-smooth',
        overlay
          ? 'bg-gray-3 border-gray-5 shadow-elevated rotate-[1.5deg] cursor-grabbing w-[300px]'
          : 'cursor-pointer border-transparent hover:bg-gray-2 hover:border-gray-4/70 hover:shadow-card active:bg-gray-3 animate-card-enter',
      )}
    >
      {/* Status + title */}
      <div className="flex items-start gap-2.5">
        <span
          className={cn(
            'mt-[5px] w-[10px] h-[10px] rounded-full flex-shrink-0 border-[1.5px]',
            status.pulse && 'animate-pulse',
          )}
          style={{ borderColor: status.color, backgroundColor: task.status === 'done' ? status.color : 'transparent' }}
        />
        <p
          title={`${getTaskLabel(task)} ${getTaskTitle(task)}`}
          className={cn(
            'flex-1 min-w-0 text-[13.5px] font-medium leading-[1.4] line-clamp-2 tracking-[-0.01em]',
            task.status === 'cancelled' ? 'text-gray-8 line-through' : 'text-gray-12',
          )}
        >
          <span className="text-gray-7 font-mono text-[11.5px] mr-1.5">{getTaskLabel(task)}</span>
          {getTaskTitle(task)}
        </p>
      </div>

      {/* Meta row */}
      <div className="flex items-center gap-2 mt-2 ml-[20px]">
        {/* Priority chip */}
        <span
          className="w-3.5 h-3.5 rounded-sm flex-shrink-0 flex items-center justify-center"
          style={{ backgroundColor: PRIORITY_COLORS[task.priority] + '25', border: `1px solid ${PRIORITY_COLORS[task.priority]}40` }}
          title={`Priority: ${task.priority}`}
        >
          {task.priority === 'urgent' && (
            <span className="block w-1 h-1 rounded-full" style={{ backgroundColor: PRIORITY_COLORS[task.priority] }} />
          )}
        </span>

        {repoName ? (
          <span className="text-[11px] text-gray-8 font-mono truncate max-w-[120px]">{repoName}</span>
        ) : (
          <span className="text-[11px] text-gray-8 flex items-center gap-1">
            <Stack size={11} weight="bold" />
            multi
          </span>
        )}

        {task.cli && <span className="text-[11px] text-gray-7 font-mono truncate max-w-[80px]">{task.cli}</span>}
        {task.cost_usd > 0 && <span className="text-[11px] text-gray-7 font-mono tabular-nums">${task.cost_usd.toFixed(2)}</span>}

        <span className="text-[11px] text-gray-7 ml-auto flex items-center gap-1 flex-shrink-0 tabular-nums">
          <Clock size={11} weight="bold" />
          {formatDistanceToNow(new Date(task.created_at), { addSuffix: false })}
        </span>

        {task.assignee_user_id && (
          <Badge tone="info" title="Assigned to a human" className="flex-shrink-0">
            <User size={10} weight="bold" /> Human
          </Badge>
        )}
        {!task.assignee_user_id && assignedAgent && (
          <Badge
            tone={assignedAgent.paused_reason ? 'warning' : 'purple'}
            title={assignedAgent.paused_reason ? `${assignedAgent.name} (paused: ${assignedAgent.paused_reason})` : `Assigned to ${assignedAgent.name}`}
            className="flex-shrink-0 max-w-[120px]"
          >
            <span aria-hidden>{assignedAgent.avatar_emoji}</span>
            <span className="truncate">{assignedAgent.name}</span>
          </Badge>
        )}
        {creatorLabel && (
          <Badge tone="neutral" title={`Created by ${creatorLabel}`} className="flex-shrink-0 max-w-[120px]">
            <User size={10} weight="bold" />
            <span className="truncate">{creatorLabel}</span>
          </Badge>
        )}
        {task.external_source &&
          (task.external_url ? (
            <a
              href={task.external_url}
              target="_blank"
              rel="noopener noreferrer"
              title={`From ${task.external_source}`}
              onClick={(e) => e.stopPropagation()}
              className="text-[10px] font-mono text-gray-7 hover:text-gray-11 transition-smooth flex-shrink-0"
            >
              {task.external_source}
            </a>
          ) : (
            <span className="text-[10px] font-mono text-gray-7 flex-shrink-0">{task.external_source}</span>
          ))}

        {task.pr_url && <GitPullRequest size={13} weight="bold" className="text-gray-8 flex-shrink-0" />}
        {task.status === 'failed' && <WarningCircle size={13} weight="fill" className="text-red-400 flex-shrink-0" />}
        {(task.status === 'planning' || task.status === 'in_progress') && !hasSubtasks && !task.assignee_user_id && (
          <Lightning size={13} weight="fill" className="text-sun-9 flex-shrink-0 animate-lightning" />
        )}
      </div>

      {/* Subtask progress */}
      {hasSubtasks && (
        <div className="mt-2.5 ml-[20px] flex items-center gap-2.5">
          <div className="flex-1 h-1 bg-gray-3 rounded-full overflow-hidden">
            <div
              className="h-full bg-green-500/80 rounded-full transition-[width] duration-300"
              style={{ width: `${task.subtask_count! > 0 ? (task.subtask_done_count! / task.subtask_count!) * 100 : 0}%` }}
            />
          </div>
          <span className="text-[11px] text-gray-8 tabular-nums flex-shrink-0">
            {task.subtask_done_count}/{task.subtask_count}
          </span>
        </div>
      )}
    </div>
  );
}
