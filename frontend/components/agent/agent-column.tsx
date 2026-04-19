'use client';

import { AgentTask, AgentColumn as AgentColumnType, COLUMN_CONFIG } from '@/lib/agent-tasks';
import { TaskCard } from './task-card';
import { useDroppable } from '@dnd-kit/core';
import { SortableContext, verticalListSortingStrategy } from '@dnd-kit/sortable';
import { Plus } from '@phosphor-icons/react';

interface AgentColumnProps {
  column: AgentColumnType;
  tasks: AgentTask[];
  onTaskClick: (task: AgentTask) => void;
  onCreateTask?: () => void;
}

export function AgentColumn({ column, tasks, onTaskClick, onCreateTask }: AgentColumnProps) {
  const { setNodeRef, isOver } = useDroppable({ id: column });
  const config = COLUMN_CONFIG[column];

  return (
    <div className="flex flex-col h-full w-[85vw] sm:w-[300px] md:w-[340px] min-w-[85vw] sm:min-w-[300px] md:min-w-[340px] sm:flex-1 snap-start border-r border-gray-3/40 last:border-r-0">
      {/* Column header */}
      <div className="flex items-center justify-between h-11 px-3 sm:px-5 flex-shrink-0">
        <div className="flex items-center gap-2.5">
          <span
            className="w-2 h-2 rounded-full flex-shrink-0"
            style={{ backgroundColor: config.color, boxShadow: `0 0 6px ${config.color}40` }}
          />
          <span className="text-[13px] font-medium text-gray-11">{config.label}</span>
          <span className="text-[13px] text-gray-8 tabular-nums">{tasks.length}</span>
        </div>
        {column === 'todo' && onCreateTask && (
          <button
            className="h-7 w-7 sm:h-6 sm:w-6 flex items-center justify-center text-gray-8 hover:text-gray-11 hover:bg-gray-3 rounded-md transition-all duration-150"
            onClick={onCreateTask}
          >
            <Plus size={15} weight="bold" />
          </button>
        )}
      </div>

      {/* Drop zone */}
      <div
        ref={setNodeRef}
        className={`flex-1 overflow-y-auto transition-all duration-150 ${
          isOver ? 'bg-gray-2/60' : ''
        }`}
      >
        <div className="p-1.5 sm:p-2 space-y-px">
          <SortableContext items={tasks.map(t => t.id)} strategy={verticalListSortingStrategy}>
            {tasks.map((task, index) => (
              <div key={task.id} style={{ animationDelay: `${index * 30}ms` }}>
                <TaskCard task={task} onClick={() => onTaskClick(task)} />
              </div>
            ))}
          </SortableContext>
        </div>
      </div>
    </div>
  );
}
