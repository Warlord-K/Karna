'use client';

import { Schedule, humanizeCron, RUN_STATUS_COLORS } from '@/lib/schedules';
import { Clock, Lightning, Pause, Play, Timer } from '@phosphor-icons/react';
import { formatDistanceToNow } from 'date-fns';

interface ScheduleCardProps {
  schedule: Schedule;
  onClick: () => void;
  onToggle: (enabled: boolean) => void;
  onTrigger: () => void;
}

export function ScheduleCard({ schedule, onClick, onToggle, onTrigger }: ScheduleCardProps) {
  const isOneShot = !!schedule.run_at;
  const lastRun = schedule.last_run;
  const lastRunColor = lastRun ? RUN_STATUS_COLORS[lastRun.status] : undefined;

  return (
    <div
      onClick={onClick}
      className="group bg-gray-2 border border-gray-3 rounded-lg p-4 hover:border-gray-5 transition-colors cursor-pointer"
    >
      {/* Top row: name + enabled toggle */}
      <div className="flex items-start justify-between gap-3 mb-2.5">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 mb-1">
            <span
              className={`w-2 h-2 rounded-full flex-shrink-0 ${
                schedule.enabled ? 'bg-green-500' : 'bg-gray-7'
              }`}
            />
            <h3 className="text-[14px] font-medium text-gray-12 truncate">
              {schedule.name}
            </h3>
          </div>
          <p className="text-[13px] text-gray-8 line-clamp-2">{schedule.prompt}</p>
        </div>

        <div className="flex items-center gap-1 flex-shrink-0" onClick={(e) => e.stopPropagation()}>
          <button
            onClick={onTrigger}
            disabled={!schedule.enabled}
            title="Run now"
            className="h-7 w-7 flex items-center justify-center text-gray-8 hover:text-sun-9 hover:bg-gray-3 rounded-md transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <Lightning size={14} weight="fill" />
          </button>
          <button
            onClick={() => onToggle(!schedule.enabled)}
            title={schedule.enabled ? 'Pause' : 'Resume'}
            className="h-7 w-7 flex items-center justify-center text-gray-8 hover:text-gray-12 hover:bg-gray-3 rounded-md transition-colors"
          >
            {schedule.enabled ? <Pause size={14} weight="fill" /> : <Play size={14} weight="fill" />}
          </button>
        </div>
      </div>

      {/* Info row */}
      <div className="flex items-center gap-3 flex-wrap text-[12px] text-gray-8">
        {/* Schedule type */}
        <span className="flex items-center gap-1">
          {isOneShot ? <Timer size={12} weight="bold" /> : <Clock size={12} weight="bold" />}
          {isOneShot
            ? `Once at ${new Date(schedule.run_at!).toLocaleString()}`
            : humanizeCron(schedule.cron_expression!)}
        </span>

        {/* Task prefix */}
        {schedule.task_prefix && (
          <span className="px-1.5 py-0.5 rounded bg-gray-3 text-gray-9 font-mono text-[11px]">
            {schedule.task_prefix}
          </span>
        )}

        {/* Max open tasks */}
        {schedule.max_open_tasks > 0 && (
          <span className="text-gray-7">max {schedule.max_open_tasks}</span>
        )}

        {/* Last run */}
        {lastRun && (
          <span className="flex items-center gap-1.5 ml-auto">
            <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: lastRunColor }} />
            {lastRun.status === 'running' ? (
              <span className="text-sun-9">Running...</span>
            ) : (
              formatDistanceToNow(new Date(lastRun.started_at), { addSuffix: true })
            )}
            {lastRun.status === 'completed' && lastRun.task_count > 0 && (
              <span className="text-gray-9">{lastRun.task_count} tasks</span>
            )}
          </span>
        )}
      </div>
    </div>
  );
}
