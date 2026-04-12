'use client';

import { useState } from 'react';
import { useSession, signOut } from 'next-auth/react';
import {
  AgentTask,
  AgentColumn as AgentColumnType,
  AGENT_COLUMNS,
  getTasksForColumn,
  getColumnForStatus,
  AgentTaskPriority,
  AgentTaskStatus,
} from '@/lib/agent-tasks';
import { useTasks, useConfig, useCreateTask, useUpdateTask, useDeleteTask } from '@/hooks/use-tasks';
import { AgentColumn } from '@/components/agent/agent-column';
import { TaskDetailModal } from '@/components/agent/task-detail-modal';
import { CreateTaskDialog, BackendConfig } from '@/components/agent/create-task-dialog';
import { Plus, ArrowsClockwise, SignOut, CircleNotch, CalendarBlank, Kanban, GitFork } from '@phosphor-icons/react';
import { SchedulesPage } from '@/components/agent/schedules-page';
import { ReposPage } from '@/components/agent/repos-page';
import { DndContext, DragEndEvent, DragOverlay, DragStartEvent, PointerSensor, TouchSensor, useSensor, useSensors } from '@dnd-kit/core';
import toast, { Toaster } from 'react-hot-toast';
import { useQueryClient } from '@tanstack/react-query';
import { taskKeys } from '@/hooks/use-tasks';
import { updateTask as updateTaskApi } from '@/lib/agent-tasks';

export default function HomePage({ authDisabled }: { authDisabled: boolean }) {
  const { data: session, status: authStatus } = useSession();
  const isReady = authDisabled || authStatus === 'authenticated';
  const [activeSection, setActiveSection] = useState<'board' | 'schedules' | 'repos'>('board');
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [selectedTask, setSelectedTask] = useState<AgentTask | null>(null);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  const queryClient = useQueryClient();
  const { data: tasks = [], isLoading } = useTasks(isReady);
  const { data: config } = useConfig(isReady);
  const createTaskMutation = useCreateTask();
  const updateTaskMutation = useUpdateTask();
  const deleteTaskMutation = useDeleteTask();

  const repos = config?.repos ?? [];
  const backends = config?.backends ?? {};
  const skills = config?.skills ?? [];
  const mcpServers = config?.mcpServers ?? [];

  // Keep selected task in sync with query data
  const selectedTaskData = selectedTask
    ? tasks.find(t => t.id === selectedTask.id) ?? selectedTask
    : null;

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
  }) => {
    await createTaskMutation.mutateAsync(data);
  };

  const handleUpdateTask = async (id: string, updates: Partial<AgentTask>) => {
    await updateTaskMutation.mutateAsync({ id, updates });
    if (selectedTask?.id === id) {
      setSelectedTask(prev => prev ? { ...prev, ...updates } : null);
    }
  };

  const handleDeleteTask = async (id: string) => {
    await deleteTaskMutation.mutateAsync(id);
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

    // Optimistic update via cache
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

  if ((!authDisabled && authStatus === 'loading') || isLoading) {
    return (
      <div className="h-screen flex items-center justify-center bg-background">
        <CircleNotch size={24} weight="bold" className="text-gray-8 animate-spin" />
      </div>
    );
  }

  const activeTask = activeId ? tasks.find(t => t.id === activeId) : null;
  const activeTasks = tasks.filter(t =>
    (t.status === 'planning' || t.status === 'in_progress') && !(t.subtask_count && t.subtask_count > 0)
  );

  return (
    <div className="h-screen flex flex-col bg-background">
      <Toaster
        position="top-right"
        toastOptions={{
          style: {
            background: '#18181b',
            color: '#ededef',
            border: '1px solid #26262b',
            borderRadius: '8px',
            fontSize: '14px',
            padding: '12px 16px',
            boxShadow: '0 4px 12px rgba(0,0,0,0.3)',
          },
        }}
      />

      {/* Header */}
      <header className="flex-shrink-0 bg-gray-1 shadow-card z-10">
        <div className="flex items-center justify-between h-14 px-3 sm:px-6">
          <div className="flex items-center gap-2 sm:gap-4">
            <div className="flex items-center gap-2 sm:gap-2.5">
              <img src="/logo-192.png" alt="Karna" width={20} height={20} />
              <span className="text-[15px] font-semibold text-gray-12 tracking-[-0.01em]">Karna</span>
            </div>
            {/* Section toggle */}
            <div className="flex items-center bg-gray-2 rounded-lg p-0.5 border border-gray-3">
              <button
                onClick={() => setActiveSection('board')}
                className={`flex items-center gap-1.5 px-2.5 h-7 rounded-md text-[12px] font-medium transition-colors ${
                  activeSection === 'board'
                    ? 'bg-gray-4 text-gray-12'
                    : 'text-gray-8 hover:text-gray-11'
                }`}
              >
                <Kanban size={13} weight="bold" />
                <span className="hidden sm:inline">Board</span>
              </button>
              <button
                onClick={() => setActiveSection('schedules')}
                className={`flex items-center gap-1.5 px-2.5 h-7 rounded-md text-[12px] font-medium transition-colors ${
                  activeSection === 'schedules'
                    ? 'bg-gray-4 text-gray-12'
                    : 'text-gray-8 hover:text-gray-11'
                }`}
              >
                <CalendarBlank size={13} weight="bold" />
                <span className="hidden sm:inline">Schedules</span>
              </button>
              <button
                onClick={() => setActiveSection('repos')}
                className={`flex items-center gap-1.5 px-2.5 h-7 rounded-md text-[12px] font-medium transition-colors ${
                  activeSection === 'repos'
                    ? 'bg-gray-4 text-gray-12'
                    : 'text-gray-8 hover:text-gray-11'
                }`}
              >
                <GitFork size={13} weight="bold" />
                <span className="hidden sm:inline">Repos</span>
              </button>
            </div>

            {activeTasks.length > 0 && (
              <div className="flex items-center gap-2 text-[13px] text-gray-9">
                <span className="w-2 h-2 rounded-full bg-sun-9 animate-pulse" />
                <span className="hidden sm:inline">{activeTasks.length} running</span>
              </div>
            )}
          </div>

          <div className="flex items-center gap-1 sm:gap-1.5">
            {session?.user?.image && (
              <img src={session.user.image} alt="" className="w-7 h-7 rounded-full mr-0.5 sm:mr-1 hidden sm:block" />
            )}
            {activeSection === 'board' && (
              <>
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
              </>
            )}
            {!authDisabled && (
              <button
                onClick={() => signOut()}
                className="h-8 w-8 flex items-center justify-center text-gray-8 hover:text-gray-11 hover:bg-gray-3 rounded-lg transition-colors ml-0.5"
              >
                <SignOut size={16} weight="bold" />
              </button>
            )}
          </div>
        </div>
      </header>

      {/* Main content */}
      <main className="flex-1 overflow-hidden">
        {activeSection === 'board' ? (
          <div className="h-full overflow-x-auto overflow-y-hidden snap-x sm:snap-none mobile-hide-scrollbar">
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
                    onTaskClick={setSelectedTask}
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
        ) : activeSection === 'schedules' ? (
          <SchedulesPage
            repos={repos}
            backends={backends}
            skills={skills}
            mcpServers={mcpServers}
          />
        ) : (
          <ReposPage />
        )}
      </main>

      <TaskDetailModal
        task={selectedTaskData}
        onClose={() => setSelectedTask(null)}
        onUpdate={handleUpdateTask}
        onDelete={handleDeleteTask}
      />

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
