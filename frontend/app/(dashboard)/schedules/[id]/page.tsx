'use client';

import { useState, useEffect, useRef } from 'react';
import { useParams, useRouter } from 'next/navigation';
import {
  Schedule,
  ScheduledRun,
  ScheduledRunLog,
  humanizeCron,
  RUN_STATUS_COLORS,
} from '@/lib/schedules';
import {
  useSchedules,
  useUpdateSchedule,
  useDeleteSchedule,
  useTriggerSchedule,
  useScheduleRuns,
  useScheduleRunLogs,
} from '@/hooks/use-schedules';
import {
  ArrowLeft, Trash, Clock, Timer, Lightning, Terminal,
  Article, Gear,
} from '@phosphor-icons/react';
import { formatDistanceToNow, format } from 'date-fns';
import toast from 'react-hot-toast';
import ReactMarkdown from 'react-markdown';

type Tab = 'details' | 'runs';

export default function ScheduleDetailPage() {
  const params = useParams();
  const router = useRouter();
  const id = params.id as string;

  const [activeTab, setActiveTab] = useState<Tab>('runs');
  const [selectedRun, setSelectedRun] = useState<ScheduledRun | null>(null);
  const [loading, setLoading] = useState(false);
  const [triggering, setTriggering] = useState(false);
  const logsEndRef = useRef<HTMLDivElement>(null);

  // Fetch the schedule from the list cache or individually
  const { data: schedules = [] } = useSchedules();
  const schedule = schedules.find(s => s.id === id) ?? null;

  const updateMutation = useUpdateSchedule();
  const deleteMutation = useDeleteSchedule();
  const triggerMutation = useTriggerSchedule();

  const { data: runs = [] } = useScheduleRuns(id, activeTab === 'runs');
  const { data: runLogs = [] } = useScheduleRunLogs(
    id,
    selectedRun?.id ?? null,
    selectedRun?.status === 'running'
  );

  useEffect(() => {
    logsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [runLogs]);

  if (!schedule) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="w-5 h-5 border-2 border-gray-7 border-t-gray-12 rounded-full animate-spin" />
      </div>
    );
  }

  const handleUpdate = async (updates: Partial<Schedule>) => {
    await updateMutation.mutateAsync({ id, updates });
  };

  const handleDelete = async () => {
    if (!confirm('Delete this schedule and all its runs?')) return;
    setLoading(true);
    try {
      await deleteMutation.mutateAsync(id);
      router.push('/schedules');
    } finally {
      setLoading(false);
    }
  };

  const handleTrigger = async () => {
    setTriggering(true);
    try {
      await triggerMutation.mutateAsync(id);
    } finally {
      setTriggering(false);
    }
  };

  const isOneShot = !!schedule.run_at;

  const tabs: { id: Tab; label: string; icon: React.ReactNode }[] = [
    { id: 'runs', label: `Runs${runs.length ? ` (${runs.length})` : ''}`, icon: <Terminal size={16} weight="bold" /> },
    { id: 'details', label: 'Details', icon: <Gear size={16} weight="bold" /> },
  ];

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-4xl mx-auto px-4 sm:px-6 py-6">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div className="flex items-center gap-3 min-w-0">
            <button
              onClick={() => router.push('/schedules')}
              className="h-8 w-8 flex items-center justify-center text-gray-8 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors flex-shrink-0"
            >
              <ArrowLeft size={16} weight="bold" />
            </button>
            <span className={`w-2.5 h-2.5 rounded-full flex-shrink-0 ${schedule.enabled ? 'bg-green-500' : 'bg-gray-7'}`} />
            <h1 className="text-[18px] font-semibold text-gray-12 truncate">{schedule.name}</h1>
            <span className="text-xs text-gray-8 flex items-center gap-1 flex-shrink-0">
              {isOneShot ? <Timer size={12} weight="bold" /> : <Clock size={12} weight="bold" />}
              {isOneShot ? 'One-shot' : humanizeCron(schedule.cron_expression!)}
            </span>
            {schedule.task_prefix && (
              <span className="px-1.5 py-0.5 rounded bg-gray-3 text-gray-9 font-mono text-[11px] flex-shrink-0">
                {schedule.task_prefix}
              </span>
            )}
          </div>

          <div className="flex items-center gap-1 sm:gap-1.5 flex-shrink-0">
            <button
              onClick={handleTrigger}
              disabled={triggering || loading}
              className="h-8 px-2 sm:px-3 text-[13px] text-gray-9 hover:text-sun-9 hover:bg-gray-3 rounded-lg transition-colors flex items-center gap-1.5 disabled:opacity-40"
            >
              {triggering ? (
                <div className="w-3.5 h-3.5 border-2 border-gray-7 border-t-sun-9 rounded-full animate-spin" />
              ) : (
                <Lightning size={14} weight="fill" />
              )}
              <span className="hidden sm:inline">{triggering ? 'Triggering...' : 'Run Now'}</span>
            </button>
            <button
              onClick={handleDelete}
              className="h-8 w-8 flex items-center justify-center text-gray-8 hover:text-red-400 hover:bg-gray-3 rounded-lg transition-colors"
            >
              <Trash size={16} weight="bold" />
            </button>
          </div>
        </div>

        {/* Tabs */}
        <div className="flex border-b border-gray-3/60 mb-6">
          {selectedRun ? (
            <button
              onClick={() => setSelectedRun(null)}
              className="flex items-center gap-1.5 px-3.5 h-11 text-[13px] font-medium text-gray-9 hover:text-gray-12 transition-colors"
            >
              <ArrowLeft size={14} weight="bold" /> Back to runs
            </button>
          ) : (
            tabs.map((tab) => (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={`flex items-center gap-1.5 sm:gap-2 px-2.5 sm:px-3.5 h-11 text-[13px] font-medium border-b-2 transition-colors whitespace-nowrap flex-shrink-0 ${
                  activeTab === tab.id
                    ? 'border-gray-12 text-gray-12'
                    : 'border-transparent text-gray-8 hover:text-gray-11'
                }`}
              >
                {tab.icon}
                {tab.label}
              </button>
            ))
          )}
        </div>

        {/* Content */}
        {selectedRun ? (
          <RunDetail run={selectedRun} logs={runLogs} logsEndRef={logsEndRef} />
        ) : activeTab === 'runs' ? (
          <RunsList runs={runs} onSelectRun={setSelectedRun} />
        ) : (
          <ScheduleDetails schedule={schedule} onUpdate={handleUpdate} />
        )}
      </div>
    </div>
  );
}

function RunsList({ runs, onSelectRun }: { runs: ScheduledRun[]; onSelectRun: (r: ScheduledRun) => void }) {
  if (runs.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-gray-8">
        <Terminal size={32} weight="thin" className="mb-3" />
        <p className="text-[14px]">No runs yet</p>
        <p className="text-[13px] mt-1.5 text-gray-7">Trigger a run or wait for the schedule</p>
      </div>
    );
  }

  return (
    <div className="space-y-px">
      {runs.map((run) => {
        const color = RUN_STATUS_COLORS[run.status];
        return (
          <button
            key={run.id}
            onClick={() => onSelectRun(run)}
            className="w-full flex flex-col sm:flex-row sm:items-center gap-1 sm:gap-3 px-3 sm:px-4 py-2.5 sm:py-0 sm:h-11 rounded-lg hover:bg-gray-3 transition-colors text-left"
          >
            <div className="flex items-center gap-2 sm:gap-3 min-w-0">
              <span
                className={`w-2.5 h-2.5 rounded-full flex-shrink-0 ${run.status === 'running' ? 'animate-pulse' : ''}`}
                style={{ backgroundColor: color }}
              />
              <span className="text-[14px] text-gray-12">
                {run.status === 'running' ? 'Running...' : run.status.charAt(0).toUpperCase() + run.status.slice(1)}
              </span>
            </div>
            <div className="flex items-center gap-2 sm:gap-3 ml-[18px] sm:ml-0 flex-shrink-0 text-xs text-gray-8">
              <span>{format(new Date(run.started_at), 'MMM d, yyyy HH:mm')}</span>
              {run.completed_at && (
                <span>{formatDistanceToNow(new Date(run.started_at), { addSuffix: false })} duration</span>
              )}
              {run.task_count > 0 && (
                <span className="text-gray-9">{run.task_count} tasks created</span>
              )}
              {run.cost_usd > 0 && (
                <span className="font-mono">${run.cost_usd.toFixed(4)}</span>
              )}
            </div>
          </button>
        );
      })}
    </div>
  );
}

function RunDetail({ run, logs, logsEndRef }: {
  run: ScheduledRun;
  logs: ScheduledRunLog[];
  logsEndRef: React.RefObject<HTMLDivElement | null>;
}) {
  const color = RUN_STATUS_COLORS[run.status];

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-3">
        <span
          className={`w-3 h-3 rounded-full ${run.status === 'running' ? 'animate-pulse' : ''}`}
          style={{ backgroundColor: color }}
        />
        <span className="text-[15px] font-medium text-gray-12">
          {run.status.charAt(0).toUpperCase() + run.status.slice(1)}
        </span>
        <span className="text-[13px] text-gray-8">
          {format(new Date(run.started_at), 'MMM d, yyyy HH:mm:ss')}
        </span>
        {run.cost_usd > 0 && (
          <span className="text-[13px] text-gray-8 font-mono ml-auto">${run.cost_usd.toFixed(4)}</span>
        )}
      </div>

      {run.summary_markdown && (
        <div className="rounded-lg bg-gray-2 border border-gray-4 p-4">
          <h3 className="text-[13px] font-medium text-gray-9 mb-3 flex items-center gap-1.5">
            <Article size={14} weight="bold" /> Summary
          </h3>
          <div className="prose prose-invert prose-sm max-w-none text-gray-11 [&_h2]:text-gray-12 [&_h3]:text-gray-12 [&_code]:text-sun-9 [&_code]:bg-gray-3 [&_code]:px-1 [&_code]:rounded [&_pre]:bg-gray-3 [&_pre]:rounded-lg [&_a]:text-sun-9">
            <ReactMarkdown>{run.summary_markdown}</ReactMarkdown>
          </div>
        </div>
      )}

      {logs.length > 0 && (
        <div>
          <h3 className="text-[13px] font-medium text-gray-9 mb-2 flex items-center gap-1.5">
            <Terminal size={14} weight="bold" /> Logs
          </h3>
          <div className="font-mono text-[12px] sm:text-[13px] leading-[1.6] sm:leading-[1.8] bg-gray-2 border border-gray-4 rounded-lg p-3 sm:p-4 overflow-x-auto max-h-[400px] overflow-y-auto">
            {logs.map((log) => {
              const time = format(new Date(log.created_at), 'HH:mm:ss');
              const levelColor = log.level === 'error' ? 'text-red-400' : log.level === 'warn' ? 'text-yellow-400' : 'text-gray-10';
              return (
                <div key={log.id} className="flex gap-3 hover:bg-gray-3 px-1.5 -mx-1.5 rounded-sm">
                  <span className="text-gray-7 flex-shrink-0 select-none">{time}</span>
                  <span className={`${levelColor} break-all`}>{log.message}</span>
                </div>
              );
            })}
            <div ref={logsEndRef} />
          </div>
        </div>
      )}

      {logs.length === 0 && !run.summary_markdown && run.status === 'running' && (
        <div className="flex items-center justify-center py-12 text-gray-8 gap-2 text-[14px]">
          <Lightning size={18} weight="fill" className="animate-pulse text-sun-9" /> Running...
        </div>
      )}
    </div>
  );
}

function ScheduleDetails({ schedule, onUpdate }: {
  schedule: Schedule;
  onUpdate: (updates: Partial<Schedule>) => Promise<void>;
}) {
  const isOneShot = !!schedule.run_at;

  return (
    <div className="space-y-4">
      <div>
        <label className="block text-[12px] font-medium text-gray-8 mb-2 uppercase tracking-wider">Prompt</label>
        <div className="rounded-lg bg-gray-2 border border-gray-4 p-3 text-[14px] text-gray-11 font-mono whitespace-pre-wrap">
          {schedule.prompt}
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3 text-[13px]">
        <div className="rounded-lg bg-gray-2 border border-gray-4 p-3">
          <span className="text-gray-8">Schedule</span>
          <p className="text-gray-12 mt-1">
            {isOneShot
              ? `Once at ${new Date(schedule.run_at!).toLocaleString()}`
              : humanizeCron(schedule.cron_expression!)}
          </p>
          {!isOneShot && (
            <p className="text-gray-7 font-mono text-[12px] mt-0.5">{schedule.cron_expression}</p>
          )}
        </div>

        <div className="rounded-lg bg-gray-2 border border-gray-4 p-3">
          <span className="text-gray-8">Priority</span>
          <p className="text-gray-12 mt-1 capitalize">{schedule.priority}</p>
        </div>

        {schedule.repos && (
          <div className="rounded-lg bg-gray-2 border border-gray-4 p-3">
            <span className="text-gray-8">Repositories</span>
            <p className="text-gray-12 mt-1 font-mono text-[12px]">
              {schedule.repos.split(',').map(r => r.trim().split('/').pop()).join(', ')}
            </p>
          </div>
        )}

        <div className="rounded-lg bg-gray-2 border border-gray-4 p-3">
          <span className="text-gray-8">Max open tasks</span>
          <p className="text-gray-12 mt-1">{schedule.max_open_tasks === 0 ? 'Unlimited' : schedule.max_open_tasks}</p>
        </div>

        {(schedule.cli || schedule.model) && (
          <div className="rounded-lg bg-gray-2 border border-gray-4 p-3">
            <span className="text-gray-8">Agent</span>
            <p className="text-gray-12 mt-1">
              {schedule.cli || 'default'} / {schedule.model || 'default'}
            </p>
          </div>
        )}

        {schedule.skills && schedule.skills.length > 0 && (
          <div className="rounded-lg bg-gray-2 border border-gray-4 p-3">
            <span className="text-gray-8">Skills</span>
            <div className="flex flex-wrap gap-1 mt-1">
              {schedule.skills.map(s => (
                <span key={s} className="px-1.5 py-0.5 rounded bg-gray-3 text-gray-11 text-[11px] font-mono">{s}</span>
              ))}
            </div>
          </div>
        )}
      </div>

      <div className="flex items-center justify-between pt-4 border-t border-gray-3">
        <div>
          <p className="text-[14px] text-gray-12 font-medium">
            {schedule.enabled ? 'Schedule is active' : 'Schedule is paused'}
          </p>
          <p className="text-[13px] text-gray-8 mt-0.5">
            {schedule.enabled ? 'Will run on its configured schedule' : 'Will not run until re-enabled'}
          </p>
        </div>
        <button
          onClick={() => onUpdate({ enabled: !schedule.enabled })}
          className={`h-9 px-4 text-[14px] font-medium rounded-lg transition-colors ${
            schedule.enabled
              ? 'text-gray-9 hover:text-gray-12 hover:bg-gray-3'
              : 'text-white bg-sun-9 hover:bg-sun-10 text-gray-1'
          }`}
        >
          {schedule.enabled ? 'Pause' : 'Enable'}
        </button>
      </div>

      <div className="text-[13px] text-gray-8 space-y-1.5 pt-4 border-t border-gray-3">
        <div className="flex items-center gap-1.5">
          <Clock size={13} weight="bold" />
          Created {format(new Date(schedule.created_at), 'MMM d, yyyy h:mm a')}
        </div>
      </div>
    </div>
  );
}
