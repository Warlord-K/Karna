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
  AgentTaskStatus,
} from '@/lib/agent-tasks';
import { useTasks, useConfig, useUsers, useAgents } from '@/hooks/use-tasks';
import { userDisplayName } from '@/lib/agent-tasks';
import { AgentColumn } from '@/components/agent/agent-column';
import { TaskCard } from '@/components/agent/task-card';
import { Plus, ArrowsClockwise } from '@phosphor-icons/react';
import {
  DndContext,
  DragEndEvent,
  DragOverlay,
  DragStartEvent,
  DropAnimation,
  PointerSensor,
  TouchSensor,
  KeyboardSensor,
  closestCorners,
  defaultDropAnimationSideEffects,
  useSensor,
  useSensors,
} from '@dnd-kit/core';
import { sortableKeyboardCoordinates } from '@dnd-kit/sortable';
import toast from 'react-hot-toast';
import { useQueryClient } from '@tanstack/react-query';
import { taskKeys } from '@/hooks/use-tasks';
import { updateTask as updateTaskApi } from '@/lib/agent-tasks';
import { Button } from '@/components/ui/button';

const dropAnimation: DropAnimation = {
  duration: 180,
  easing: 'cubic-bezier(0.16, 1, 0.3, 1)',
  sideEffects: defaultDropAnimationSideEffects({
    styles: { active: { opacity: '0.35' } },
  }),
};

export default function BoardPage() {
  const authDisabled = useAuthDisabled();
  const { data: session, status: authStatus } = useSession();
  const isReady = authDisabled || authStatus === 'authenticated';
  const router = useRouter();

  const [activeId, setActiveId] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  const queryClient = useQueryClient();
  const { data: tasks = [] } = useTasks(isReady);
  const { data: config } = useConfig(isReady);
  const sharedWorkspace = config?.sharedWorkspace ?? false;
  const { data: users = [] } = useUsers(isReady && sharedWorkspace);
  const { data: agents = [] } = useAgents(isReady);
  const currentUserId = (session?.user as { id?: string } | undefined)?.id;

  const getCreatorLabel = (task: AgentTask): string | null => {
    if (!sharedWorkspace) return null;
    if (!task.user_id || task.user_id === currentUserId) return null;
    const u = users.find((u) => u.id === task.user_id);
    return u ? userDisplayName(u) : null;
  };

  const getAssignedAgent = (task: AgentTask) => {
    if (!task.assigned_agent_id) return null;
    return agents.find((a) => a.id === task.assigned_agent_id) ?? null;
  };

  const sensors = useSensors(
    useSensor(TouchSensor, { activationConstraint: { delay: 200, tolerance: 5 } }),
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates })
  );

  const handleRefresh = async () => {
    setRefreshing(true);
    await queryClient.invalidateQueries({ queryKey: taskKeys.lists() });
    setTimeout(() => setRefreshing(false), 500);
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

  const handleNewTask = () => {
    router.push('/tasks/new');
  };

  const activeTask = activeId ? tasks.find(t => t.id === activeId) : null;

  return (
    <div className="h-full flex flex-col">
      {/* Board action bar */}
      <div className="flex items-center justify-end gap-1.5 px-3 sm:px-5 py-2 flex-shrink-0">
        <Button
          variant="ghost"
          size="icon"
          onClick={handleRefresh}
          aria-label="Refresh board"
          title="Refresh"
        >
          <ArrowsClockwise size={16} weight="bold" className={refreshing ? 'animate-spin' : ''} />
        </Button>
        <Button variant="primary" size="md" onClick={handleNewTask}>
          <Plus size={15} weight="bold" />
          <span className="hidden sm:inline">New Task</span>
        </Button>
      </div>

      <div className="flex-1 overflow-x-auto overflow-y-hidden snap-x sm:snap-none mobile-hide-scrollbar">
        <DndContext
          sensors={sensors}
          collisionDetection={closestCorners}
          onDragStart={handleDragStart}
          onDragEnd={handleDragEnd}
          onDragCancel={() => setActiveId(null)}
        >
          <div className="flex gap-0 h-full min-w-max sm:min-w-0">
            {AGENT_COLUMNS.map((column) => (
              <AgentColumn
                key={column}
                column={column}
                tasks={getTasksForColumn(tasks, column)}
                onTaskClick={handleTaskClick}
                onCreateTask={column === 'todo' ? handleNewTask : undefined}
                getCreatorLabel={getCreatorLabel}
                getAssignedAgent={getAssignedAgent}
              />
            ))}
          </div>

          <DragOverlay dropAnimation={dropAnimation}>
            {activeTask && (
              <TaskCard
                task={activeTask}
                onClick={() => {}}
                creatorLabel={getCreatorLabel(activeTask)}
                assignedAgent={getAssignedAgent(activeTask)}
                overlay
              />
            )}
          </DragOverlay>
        </DndContext>
      </div>
    </div>
  );
}
