'use client';

import { AgentTask, PRIORITY_COLORS, getTaskLabel, getTaskTitle } from '@/lib/agent-tasks';
import { GitPullRequest, WarningCircle, Lightning, Stack, Clock } from '@phosphor-icons/react';
import { formatDistanceToNow } from 'date-fns';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';

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
}

export function TaskCard({ task, onClick }: TaskCardProps) {
  const {
    attributes, listeners, setNodeRef, transform, transition, isDragging,
  } = useSortable({ id: task.id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.4 : 1,
  };

  const status = statusConfig[task.status];
  const repoName = task.repo ? (task.repo.split('/').pop() || task.repo) : null;
  const hasSubtasks = (task.subtask_count ?? 0) > 0;

  return (
    <div
      ref={setNodeRef}
      style={style}
      {...attributes}
      {...listeners}
      onClick={onClick}
      className="group card-hover-glow px-3 sm:px-4 py-3 rounded-lg cursor-pointer border border-transparent hover:bg-gray-2 hover:shadow-card-glow hover:border-gray-4/60 active:bg-gray-3 transition-all duration-150 animate-card-enter"
    >
      {/* Status + title */}
      <div className="flex items-start gap-2.5">
        <span
          className={`mt-[5px] w-[11px] h-[11px] rounded-full flex-shrink-0 border-[1.5px] ${status.pulse ? 'animate-pulse' : ''}`}
          style={{ borderColor: status.color, backgroundColor: task.status === 'done' ? status.color : 'transparent' }}
        />
        <div className="flex-1 min-w-0">
          <p
            title={`${getTaskLabel(task)} ${getTaskTitle(task)}`}
            className={`text-[14px] font-medium leading-[1.45] line-clamp-2 tracking-[-0.01em] ${task.status === 'cancelled' ? 'text-gray-8 line-through' : 'text-gray-12'}`}
          >
            <span className="text-gray-7 font-mono text-[12px] mr-1.5">{getTaskLabel(task)}</span>
            {getTaskTitle(task)}
          </p>
        </div>
      </div>

      {/* Meta row */}
      <div className="flex items-center gap-2 sm:gap-2.5 mt-2 ml-[21px]">
        {/* Priority */}
        <span
          className="w-3.5 h-3.5 rounded-sm flex-shrink-0"
          style={{ backgroundColor: PRIORITY_COLORS[task.priority] + '25', border: `1px solid ${PRIORITY_COLORS[task.priority]}40` }}
        >
          <span className="block w-full h-full flex items-center justify-center">
            {task.priority === 'urgent' && <span className="block w-1 h-1 rounded-full" style={{ backgroundColor: PRIORITY_COLORS[task.priority] }} />}
          </span>
        </span>

        {repoName ? (
          <span className="text-xs text-gray-8 font-mono truncate">{repoName}</span>
        ) : (
          <span className="text-xs text-gray-8 flex items-center gap-1">
            <Stack size={12} weight="bold" />
            multi
          </span>
        )}

        {task.cli && (
          <span className="text-xs text-gray-7 font-mono">{task.cli}</span>
        )}

        {task.cost_usd > 0 && (
          <span className="text-xs text-gray-7 font-mono">${task.cost_usd.toFixed(2)}</span>
        )}

        <span className="text-xs text-gray-7 ml-auto flex items-center gap-1 flex-shrink-0">
          <Clock size={12} weight="bold" />
          {formatDistanceToNow(new Date(task.created_at), { addSuffix: false })}
        </span>

        {task.pr_url && <GitPullRequest size={14} weight="bold" className="text-gray-8 flex-shrink-0" />}
        {task.status === 'failed' && <WarningCircle size={14} weight="fill" className="text-red-400 flex-shrink-0" />}
        {(task.status === 'planning' || task.status === 'in_progress') && !hasSubtasks && (
          <Lightning size={14} weight="fill" className="text-sun-9 flex-shrink-0 animate-lightning" />
        )}
      </div>

      {/* Subtask progress */}
      {hasSubtasks && (
        <div className="mt-2.5 ml-[21px]">
          <div className="flex items-center gap-2.5">
            <div className="flex-1 h-1.5 bg-gray-3 rounded-full overflow-hidden">
              <div
                className="h-full bg-green-500 rounded-full transition-all duration-300"
                style={{ width: `${task.subtask_count! > 0 ? (task.subtask_done_count! / task.subtask_count!) * 100 : 0}%` }}
              />
            </div>
            <span className="text-xs text-gray-8 tabular-nums flex-shrink-0">
              {task.subtask_done_count}/{task.subtask_count}
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
