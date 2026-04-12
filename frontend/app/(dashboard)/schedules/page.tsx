'use client';

import { useState } from 'react';
import { useRouter } from 'next/navigation';
import { useSession } from 'next-auth/react';
import { useAuthDisabled } from '@/lib/auth-context';
import { useConfig } from '@/hooks/use-tasks';
import {
  useSchedules,
  useCreateSchedule,
  useUpdateSchedule,
  useTriggerSchedule,
} from '@/hooks/use-schedules';
import { Schedule } from '@/lib/schedules';
import { ScheduleCard } from '@/components/agent/schedule-card';
import { CreateScheduleDialog } from '@/components/agent/create-schedule-dialog';
import { Plus, CalendarBlank } from '@phosphor-icons/react';
import toast from 'react-hot-toast';

export default function SchedulesListPage() {
  const authDisabled = useAuthDisabled();
  const { status: authStatus } = useSession();
  const isReady = authDisabled || authStatus === 'authenticated';
  const router = useRouter();

  const [createOpen, setCreateOpen] = useState(false);

  const { data: config } = useConfig(isReady);
  const { data: schedules = [], isLoading } = useSchedules();
  const createMutation = useCreateSchedule();
  const updateMutation = useUpdateSchedule();
  const triggerMutation = useTriggerSchedule();

  const repos = config?.repos ?? [];
  const backends = config?.backends ?? {};
  const skills = config?.skills ?? [];
  const mcpServers = config?.mcpServers ?? [];

  const handleCreate = async (data: Parameters<typeof createMutation.mutateAsync>[0]) => {
    await createMutation.mutateAsync(data);
  };

  const handleToggle = async (schedule: Schedule, enabled: boolean) => {
    await updateMutation.mutateAsync({ id: schedule.id, updates: { enabled } });
    toast.success(enabled ? 'Schedule enabled' : 'Schedule paused');
  };

  const handleTrigger = async (id: string) => {
    await triggerMutation.mutateAsync(id);
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="w-5 h-5 border-2 border-gray-7 border-t-gray-12 rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-3xl mx-auto px-4 sm:px-6 py-6">
        <div className="flex items-center justify-between mb-6">
          <div>
            <h2 className="text-[18px] font-semibold text-gray-12 tracking-[-0.02em]">Schedules</h2>
            <p className="text-[13px] text-gray-8 mt-0.5">Automated agent runs on a recurring or one-shot basis</p>
          </div>
          <button
            onClick={() => setCreateOpen(true)}
            className="h-8 sm:w-auto px-3.5 text-[13px] font-medium text-white bg-sun-9 hover:bg-sun-10 text-gray-1 rounded-lg transition-colors flex items-center gap-1.5"
          >
            <Plus size={15} weight="bold" />
            <span className="hidden sm:inline">New Schedule</span>
          </button>
        </div>

        {schedules.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-20 text-gray-8">
            <CalendarBlank size={48} weight="thin" className="mb-4" />
            <p className="text-[15px] font-medium text-gray-10">No schedules yet</p>
            <p className="text-[13px] mt-1.5 max-w-xs text-center">
              Create a schedule to run automated agent tasks on a recurring basis — bug hunting, security scans, code reviews, and more.
            </p>
            <button
              onClick={() => setCreateOpen(true)}
              className="h-9 px-4 mt-4 text-[14px] font-medium text-white bg-sun-9 hover:bg-sun-10 text-gray-1 rounded-lg transition-colors flex items-center gap-1.5"
            >
              <Plus size={15} weight="bold" /> Create schedule
            </button>
          </div>
        ) : (
          <div className="space-y-2">
            {schedules.map((schedule) => (
              <ScheduleCard
                key={schedule.id}
                schedule={schedule}
                onClick={() => router.push(`/schedules/${schedule.id}`)}
                onToggle={(enabled) => handleToggle(schedule, enabled)}
                onTrigger={() => handleTrigger(schedule.id)}
                isTriggering={triggerMutation.isPending && triggerMutation.variables === schedule.id}
              />
            ))}
          </div>
        )}
      </div>

      <CreateScheduleDialog
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        repos={repos}
        backends={backends}
        skills={skills}
        mcpServers={mcpServers}
        onCreateSchedule={handleCreate}
      />
    </div>
  );
}
