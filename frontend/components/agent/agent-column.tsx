'use client';

import { AgentTask, AgentColumn as AgentColumnType, COLUMN_CONFIG } from '@/lib/agent-tasks';
import { AgentProfile } from '@/lib/agents';
import { TaskCard } from './task-card';
import { useDroppable } from '@dnd-kit/core';
import { SortableContext, verticalListSortingStrategy } from '@dnd-kit/sortable';
import { Plus } from '@phosphor-icons/react';
import { cn } from '@/lib/utils';

interface AgentColumnProps {
  column: AgentColumnType;
  tasks: AgentTask[];
  onTaskClick: (task: AgentTask) => void;
  onCreateTask?: () => void;
  /** Optional resolver for the creator badge shown in shared-workspace mode. */
  getCreatorLabel?: (task: AgentTask) => string | null;
  /** Optional resolver for the assigned-agent badge. */
  getAssignedAgent?: (task: AgentTask) => AgentProfile | null;
}

export function AgentColumn({ column, tasks, onTaskClick, onCreateTask, getCreatorLabel, getAssignedAgent }: AgentColumnProps) {
  const { setNodeRef, isOver } = useDroppable({ id: column });
  const config = COLUMN_CONFIG[column];

  return (
    <div className="flex flex-col h-full w-[85vw] sm:w-[300px] md:w-[336px] min-w-[85vw] sm:min-w-[300px] md:min-w-[336px] sm:flex-1 snap-start border-r border-gray-3/50 last:border-r-0">
      {/* Column header */}
      <div className="flex items-center justify-between h-11 px-3 sm:px-4 flex-shrink-0">
        <div className="flex items-center gap-2">
          <span
            className="w-2 h-2 rounded-full flex-shrink-0"
            style={{ backgroundColor: config.color }}
          />
          <span className="text-[12px] font-semibold text-gray-11 tracking-[-0.01em]">{config.label}</span>
          <span className="inline-flex items-center justify-center min-w-[18px] h-[18px] px-1 rounded-md bg-gray-3 text-[11px] text-gray-9 tabular-nums">
            {tasks.length}
          </span>
        </div>
        {column === 'todo' && onCreateTask && (
          <button
            className="h-6 w-6 flex items-center justify-center text-gray-8 hover:text-gray-12 hover:bg-gray-3 rounded-md transition-smooth focus-ring press-scale"
            onClick={onCreateTask}
            aria-label="New task"
          >
            <Plus size={15} weight="bold" />
          </button>
        )}
      </div>

      {/* Drop zone */}
      <div
        ref={setNodeRef}
        className={cn(
          'flex-1 overflow-y-auto px-1.5 sm:px-2 pb-2 transition-smooth rounded-md',
          isOver && 'bg-sun-9/[0.04] ring-1 ring-inset ring-sun-9/20',
        )}
      >
        <div className="space-y-px pt-0.5">
          <SortableContext items={tasks.map((t) => t.id)} strategy={verticalListSortingStrategy}>
            {tasks.map((task, index) => (
              <div key={task.id} style={{ animationDelay: `${Math.min(index, 8) * 25}ms` }}>
                <TaskCard
                  task={task}
                  onClick={() => onTaskClick(task)}
                  creatorLabel={getCreatorLabel ? getCreatorLabel(task) : null}
                  assignedAgent={getAssignedAgent ? getAssignedAgent(task) : null}
                />
              </div>
            ))}
          </SortableContext>

          {tasks.length === 0 && (
            <div
              className={cn(
                'mt-1 flex flex-col items-center justify-center gap-1 rounded-lg border border-dashed py-8 text-center transition-smooth',
                isOver ? 'border-sun-9/40 text-sun-10' : 'border-gray-3 text-gray-7',
              )}
            >
              {column === 'todo' && onCreateTask ? (
                <button
                  onClick={onCreateTask}
                  className="flex items-center gap-1.5 text-[12px] text-gray-8 hover:text-gray-11 transition-smooth focus-ring rounded-md px-2 py-1"
                >
                  <Plus size={13} weight="bold" /> Add a task
                </button>
              ) : (
                <span className="text-[12px]">{isOver ? 'Drop here' : 'No tasks'}</span>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
