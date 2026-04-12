'use client';

import { useState, useEffect, useRef } from 'react';
import {
  Schedule,
  ScheduledRun,
  ScheduledRunLog,
  humanizeCron,
  RUN_STATUS_COLORS,
} from '@/lib/schedules';
import { useScheduleRuns, useScheduleRunLogs } from '@/hooks/use-schedules';
import {
  X, Trash, Clock, Timer, Lightning, Terminal,
  ArrowLeft, Article, Gear,
} from '@phosphor-icons/react';
import { formatDistanceToNow, format } from 'date-fns';
import toast from 'react-hot-toast';
import ReactMarkdown from 'react-markdown';

type Tab = 'details' | 'runs';

interface ScheduleDetailModalProps {
  schedule: Schedule | null;
  onClose: () => void;
  onUpdate: (id: string, updates: Partial<Schedule>) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  onTrigger: (id: string) => Promise<void>;
}

export function ScheduleDetailModal({
  schedule, onClose, onUpdate, onDelete, onTrigger,
}: ScheduleDetailModalProps) {
  const [activeTab, setActiveTab] = useState<Tab>('runs');
  const [selectedRun, setSelectedRun] = useState<ScheduledRun | null>(null);
  const [loading, setLoading] = useState(false);
  const logsEndRef = useRef<HTMLDivElement>(null);

  const prevScheduleId = useRef<string | null>(null);

  useEffect(() => {
    if (schedule && schedule.id !== prevScheduleId.current) {
      prevScheduleId.current = schedule.id;
      setActiveTab('runs');
      setSelectedRun(null);
    }
    if (!schedule) {
      prevScheduleId.current = null;
    }
  }, [schedule]);

  const scheduleId = schedule?.id ?? null;

  const { data: runs = [] } = useScheduleRuns(scheduleId, activeTab === 'runs');
  const { data: runLogs = [] } = useScheduleRunLogs(
    scheduleId,
    selectedRun?.id ?? null,
    selectedRun?.status === 'running'
  );

  useEffect(() => {
    logsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [runLogs]);

  if (!schedule) return null;

  const handleDelete = async () => {
    if (!confirm('Delete this schedule and all its runs?')) return;
    setLoading(true);
    try { await onDelete(schedule.id); onClose(); } finally { setLoading(false); }
  };

  const handleTrigger = async () => {
    setLoading(true);
    try {
      await onTrigger(schedule.id);
      toast.success('Schedule triggered');
    } catch {
      toast.error('Failed to trigger');
    } finally {
      setLoading(false);
    }
  };

  const isOneShot = !!schedule.run_at;

  const tabs: { id: Tab; label: string; icon: React.ReactNode }[] = [
    { id: 'runs',    label: `Runs${runs.length ? ` (${runs.length})` : ''}`, icon: <Terminal size={16} weight="bold" /> },
    { id: 'details', label: 'Details', icon: <Gear size={16} weight="bold" /> },
  ];

  return (
    <div className="fixed inset-0 z-50 flex items-end sm:items-center justify-center">
      <div className="fixed inset-0 bg-black/50" onClick={onClose} />
      <div className="relative bg-gray-1 rounded-t-2xl sm:rounded-xl shadow-modal w-full sm:max-w-4xl sm:mx-6 h-[95vh] sm:h-auto sm:max-h-[85vh] flex flex-col overflow-hidden">
        {/* Mobile drag handle */}
        <div className="sm:hidden flex justify-center pt-2 pb-0 flex-shrink-0">
          <div className="w-8 h-1 rounded-full bg-gray-6" />
        </div>

        {/* Header */}
        <div className="flex items-center justify-between px-4 sm:px-6 h-12 sm:h-14 border-b border-gray-3/60 flex-shrink-0">
          <div className="flex items-center gap-2 sm:gap-3 flex-1 min-w-0">
            <span className={`w-2.5 h-2.5 rounded-full flex-shrink-0 ${schedule.enabled ? 'bg-green-500' : 'bg-gray-7'}`} />
            <h2 className="text-[15px] font-semibold text-gray-12 truncate">{schedule.name}</h2>
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
              disabled={loading || !schedule.enabled}
              className="h-8 px-2 sm:px-3 text-[13px] text-gray-9 hover:text-sun-9 hover:bg-gray-3 rounded-lg transition-colors flex items-center gap-1.5 disabled:opacity-40"
            >
              <Lightning size={14} weight="fill" /> <span className="hidden sm:inline">Run Now</span>
            </button>
            <button onClick={handleDelete} className="h-8 w-8 flex items-center justify-center text-gray-8 hover:text-red-400 hover:bg-gray-3 rounded-lg transition-colors">
              <Trash size={16} weight="bold" />
            </button>
            <button onClick={onClose} className="h-8 w-8 flex items-center justify-center text-gray-8 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors">
              <X size={16} weight="bold" />
            </button>
          </div>
        </div>

        {/* Tabs */}
        <div className="flex border-b border-gray-3/60 px-3 sm:px-6 flex-shrink-0">
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
                <span className="hidden sm:inline">{tab.label}</span>
              </button>
            ))
          )}
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4 sm:p-6">
          {selectedRun ? (
            <RunDetail run={selectedRun} logs={runLogs} logsEndRef={logsEndRef} />
          ) : activeTab === 'runs' ? (
            <RunsList runs={runs} onSelectRun={setSelectedRun} />
          ) : (
            <ScheduleDetails schedule={schedule} onUpdate={onUpdate} />
          )}
        </div>
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
      {/* Run status header */}
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

      {/* Summary markdown */}
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

      {/* Logs */}
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
  onUpdate: (id: string, updates: Partial<Schedule>) => Promise<void>;
}) {
  const isOneShot = !!schedule.run_at;

  return (
    <div className="space-y-4">
      {/* Prompt */}
      <div>
        <label className="block text-[12px] font-medium text-gray-8 mb-2 uppercase tracking-wider">Prompt</label>
        <div className="rounded-lg bg-gray-2 border border-gray-4 p-3 text-[14px] text-gray-11 font-mono whitespace-pre-wrap">
          {schedule.prompt}
        </div>
      </div>

      {/* Config grid */}
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

      {/* Toggle enabled */}
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
          onClick={() => onUpdate(schedule.id, { enabled: !schedule.enabled })}
          className={`h-9 px-4 text-[14px] font-medium rounded-lg transition-colors ${
            schedule.enabled
              ? 'text-gray-9 hover:text-gray-12 hover:bg-gray-3'
              : 'text-white bg-sun-9 hover:bg-sun-10 text-gray-1'
          }`}
        >
          {schedule.enabled ? 'Pause' : 'Enable'}
        </button>
      </div>

      {/* Timestamps */}
      <div className="text-[13px] text-gray-8 space-y-1.5 pt-4 border-t border-gray-3">
        <div className="flex items-center gap-1.5">
          <Clock size={13} weight="bold" />
          Created {format(new Date(schedule.created_at), 'MMM d, yyyy h:mm a')}
        </div>
      </div>
    </div>
  );
}
