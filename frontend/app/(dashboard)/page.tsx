'use client';

import { useState } from 'react';
import { useSession } from 'next-auth/react';
import { useRouter } from 'next/navigation';
import { useAuthDisabled } from '@/lib/auth-context';
import {
  AgentTask,
  AgentColumn as AgentColumnType,
  AGENT_COLUMNS,
  getTasksForColumn,
  getColumnForStatus,
  AgentTaskPriority,
  AgentTaskStatus,
} from '@/lib/agent-tasks';
import { useTasks, useConfig } from '@/hooks/use-tasks';
import { createTaskWithImages } from '@/lib/agent-tasks';
import { AgentColumn } from '@/components/agent/agent-column';
import { CreateTaskDialog, BackendConfig } from '@/components/agent/create-task-dialog';
import { Plus, ArrowsClockwise } from '@phosphor-icons/react';
import { DndContext, DragEndEvent, DragOverlay, DragStartEvent, PointerSensor, TouchSensor, useSensor, useSensors } from '@dnd-kit/core';
import toast from 'react-hot-toast';
import { useQueryClient } from '@tanstack/react-query';
import { taskKeys } from '@/hooks/use-tasks';
import { updateTask as updateTaskApi } from '@/lib/agent-tasks';

export default function BoardPage() {
  const authDisabled = useAuthDisabled();
  const { status: authStatus } = useSession();
  const isReady = authDisabled || authStatus === 'authenticated';
  const router = useRouter();

  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  const queryClient = useQueryClient();
  const { data: tasks = [] } = useTasks(isReady);
  const { data: config } = useConfig(isReady);

  const repos = config?.repos ?? [];
  const backends = config?.backends ?? {};

  const sensors = useSensors(
    useSensor(TouchSensor, { activationConstraint: { delay: 200, tolerance: 5 } }),
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } })
  );

  const handleRefresh = async () => {
    setRefreshing(true);
    await queryClient.invalidateQueries({ queryKey: taskKeys.lists() });
    setTimeout(() => setRefreshing(false), 500);
  };

  const handleCreateTask = async (data: {
    title: string;
    description: string;
    repo: string | null;
    priority: AgentTaskPriority;
    cli: string | null;
    model: string | null;
  }, images: File[] = []) => {
    await createTaskWithImages(data, images);
    queryClient.invalidateQueries({ queryKey: taskKeys.lists() });
    toast.success('Task created');
  };

  const handleDragStart = (event: DragStartEvent) => {
    setActiveId(event.active.id as string);
  };

  const handleDragEnd = async (event: DragEndEvent) => {
    const { active, over } = event;
    setActiveId(null);
    if (!over) return;

    const task = tasks.find(t => t.id === active.id);
    if (!task) return;

    const targetColumn = over.id as AgentColumnType;
    const currentColumn = getColumnForStatus(task.status);
    if (currentColumn === targetColumn) return;

    const statusMap: Record<AgentColumnType, AgentTaskStatus> = {
      todo: 'todo',
      plan: 'plan_review',
      in_progress: 'in_progress',
      review: 'review',
      done: 'done',
      failed: 'failed',
    };

    const newStatus = statusMap[targetColumn];

    queryClient.setQueryData<AgentTask[]>(
      taskKeys.lists(),
      (old) => old?.map(t => t.id === task.id ? { ...t, status: newStatus } : t) ?? []
    );

    try {
      await updateTaskApi(task.id, { status: newStatus });
    } catch {
      toast.error('Failed to move task');
      queryClient.invalidateQueries({ queryKey: taskKeys.lists() });
    }
  };

  const handleTaskClick = (task: AgentTask) => {
    router.push(`/tasks/${task.id}`);
  };

  const activeTask = activeId ? tasks.find(t => t.id === activeId) : null;

  return (
    <div className="h-full flex flex-col">
      {/* Board action bar */}
      <div className="flex items-center justify-end gap-1 sm:gap-1.5 px-3 sm:px-6 py-2 flex-shrink-0">
        <button
          onClick={handleRefresh}
          className="h-8 w-8 sm:px-2.5 text-[13px] text-gray-9 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors flex items-center justify-center gap-1.5"
        >
          <ArrowsClockwise size={16} weight="bold" className={refreshing ? 'animate-spin' : ''} />
        </button>
        <button
          onClick={() => setCreateDialogOpen(true)}
          className="h-8 w-8 sm:w-auto sm:px-3.5 text-[13px] font-medium text-white bg-sun-9 hover:bg-sun-10 text-gray-1 rounded-lg transition-colors flex items-center justify-center gap-1.5"
        >
          <Plus size={15} weight="bold" />
          <span className="hidden sm:inline">New Task</span>
        </button>
      </div>

      <div className="flex-1 overflow-x-auto overflow-y-hidden snap-x sm:snap-none mobile-hide-scrollbar">
        <DndContext
          sensors={sensors}
          onDragStart={handleDragStart}
          onDragEnd={handleDragEnd}
        >
          <div className="flex gap-0 h-full min-w-max sm:min-w-0">
            {AGENT_COLUMNS.map((column) => (
              <AgentColumn
                key={column}
                column={column}
                tasks={getTasksForColumn(tasks, column)}
                onTaskClick={handleTaskClick}
                onCreateTask={column === 'todo' ? () => setCreateDialogOpen(true) : undefined}
              />
            ))}
          </div>

          <DragOverlay>
            {activeTask && (
              <div className="bg-gray-2 rounded-lg p-4 shadow-elevated w-[280px] sm:w-[320px]">
                <span className="text-[14px] font-medium text-gray-12">{activeTask.title}</span>
              </div>
            )}
          </DragOverlay>
        </DndContext>
      </div>

      <CreateTaskDialog
        open={createDialogOpen}
        onClose={() => setCreateDialogOpen(false)}
        repos={repos}
        backends={backends}
        onCreateTask={handleCreateTask}
      />
    </div>
  );
}
