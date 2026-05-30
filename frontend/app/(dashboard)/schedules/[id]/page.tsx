'use client';

import { useState, useEffect, useRef, useMemo } from 'react';
import { useParams, useRouter } from 'next/navigation';
import { useSession } from 'next-auth/react';
import { useAuthDisabled } from '@/lib/auth-context';
import { useConfig } from '@/hooks/use-tasks';
import { BackendConfig } from '@/components/agent/create-task-dialog';
import {
  Schedule,
  SchedulePriority,
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

const PRIORITIES: { value: SchedulePriority; label: string }[] = [
  { value: 'urgent', label: 'Urgent' },
  { value: 'high',   label: 'High' },
  { value: 'medium', label: 'Medium' },
  { value: 'low',    label: 'Low' },
];

const CRON_PRESETS = [
  { label: 'Every 30 min',     value: '*/30 * * * *' },
  { label: 'Every hour',       value: '0 * * * *' },
  { label: 'Every 4 hours',    value: '0 */4 * * *' },
  { label: 'Every 12 hours',   value: '0 */12 * * *' },
  { label: 'Daily at 9am',     value: '0 9 * * *' },
  { label: 'Weekdays at 9am',  value: '0 9 * * 1-5' },
  { label: 'Weekly (Mon 9am)', value: '0 9 * * 1' },
  { label: 'Custom',           value: '' },
];

// datetime-local input value <-> ISO 8601 with timezone.
// run_at is stored as UTC ISO 8601; the input control wants local time `YYYY-MM-DDTHH:mm`.
function isoToLocalInput(iso: string | null | undefined): string {
  if (!iso) return '';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '';
  const pad = (n: number) => n.toString().padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function localInputToIso(local: string): string | null {
  if (!local) return null;
  const d = new Date(local);
  if (Number.isNaN(d.getTime())) return null;
  return d.toISOString();
}

type Tab = 'details' | 'runs';

export default function ScheduleDetailPage() {
  const params = useParams();
  const router = useRouter();
  const id = params.id as string;

  const authDisabled = useAuthDisabled();
  const { status: authStatus } = useSession();
  const isReady = authDisabled || authStatus === 'authenticated';

  const [activeTab, setActiveTab] = useState<Tab>('runs');
  const [selectedRun, setSelectedRun] = useState<ScheduledRun | null>(null);
  const [loading, setLoading] = useState(false);
  const [triggering, setTriggering] = useState(false);
  const logsEndRef = useRef<HTMLDivElement>(null);

  // Fetch the schedule from the list cache or individually
  const { data: schedules = [] } = useSchedules();
  const { data: config } = useConfig(isReady);
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
          <ScheduleDetails
            schedule={schedule}
            onUpdate={handleUpdate}
            repos={config?.repos ?? []}
            backends={config?.backends ?? {}}
            allSkills={config?.skills ?? []}
            allMcpServers={config?.mcpServers ?? []}
          />
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

function ScheduleDetails({
  schedule, onUpdate, repos, backends, allSkills, allMcpServers,
}: {
  schedule: Schedule;
  onUpdate: (updates: Partial<Schedule>) => Promise<void>;
  repos: string[];
  backends: Record<string, BackendConfig>;
  allSkills: string[];
  allMcpServers: string[];
}) {
  const backendNames = Object.keys(backends);
  const initialIsOneShot = !!schedule.run_at;

  const [name, setName] = useState(schedule.name);
  const [prompt, setPrompt] = useState(schedule.prompt);
  const [mode, setMode] = useState<'cron' | 'once'>(initialIsOneShot ? 'once' : 'cron');
  const initialCron = schedule.cron_expression ?? '0 */4 * * *';
  const isInitialCronPreset = CRON_PRESETS.some(p => p.value === initialCron);
  const [cronPreset, setCronPreset] = useState(isInitialCronPreset ? initialCron : '');
  const [customCron, setCustomCron] = useState(isInitialCronPreset ? '' : initialCron);
  const [runAtLocal, setRunAtLocal] = useState(isoToLocalInput(schedule.run_at));
  const [selectedRepos, setSelectedRepos] = useState<string[]>(
    schedule.repos ? schedule.repos.split(',').map(r => r.trim()).filter(Boolean) : []
  );
  const [selectedSkills, setSelectedSkills] = useState<string[]>(schedule.skills ?? []);
  const [selectedMcp, setSelectedMcp] = useState<string[]>(schedule.mcp_servers ?? []);
  const [maxOpenTasks, setMaxOpenTasks] = useState(schedule.max_open_tasks);
  const [taskPrefix, setTaskPrefix] = useState(schedule.task_prefix ?? '');
  const [priority, setPriority] = useState<SchedulePriority>(schedule.priority);
  const [cli, setCli] = useState(schedule.cli ?? backendNames[0] ?? 'claude');
  const [model, setModel] = useState(schedule.model ?? backends[schedule.cli ?? '']?.default_model ?? '');
  const [saving, setSaving] = useState(false);

  // Reset form when navigating between schedules
  const lastScheduleId = useRef(schedule.id);
  useEffect(() => {
    if (lastScheduleId.current !== schedule.id) {
      lastScheduleId.current = schedule.id;
      setName(schedule.name);
      setPrompt(schedule.prompt);
      setMode(schedule.run_at ? 'once' : 'cron');
      const cron = schedule.cron_expression ?? '0 */4 * * *';
      const preset = CRON_PRESETS.some(p => p.value === cron);
      setCronPreset(preset ? cron : '');
      setCustomCron(preset ? '' : cron);
      setRunAtLocal(isoToLocalInput(schedule.run_at));
      setSelectedRepos(schedule.repos ? schedule.repos.split(',').map(r => r.trim()).filter(Boolean) : []);
      setSelectedSkills(schedule.skills ?? []);
      setSelectedMcp(schedule.mcp_servers ?? []);
      setMaxOpenTasks(schedule.max_open_tasks);
      setTaskPrefix(schedule.task_prefix ?? '');
      setPriority(schedule.priority);
      setCli(schedule.cli ?? backendNames[0] ?? 'claude');
      setModel(schedule.model ?? backends[schedule.cli ?? '']?.default_model ?? '');
    }
  }, [schedule, backendNames, backends]);

  // When CLI changes, reset model to the new CLI's default. User can then type any model.
  // Skips on initial mount because the load effect above already set model from the schedule row.
  const cliRef = useRef(cli);
  useEffect(() => {
    if (cliRef.current === cli) return;
    cliRef.current = cli;
    const backend = backends[cli];
    if (backend) setModel(backend.default_model || backend.models[0] || '');
  }, [cli, backends]);

  const cronValue = cronPreset || customCron;
  const currentModels = backends[cli]?.models ?? (model ? [model] : []);

  const desiredUpdates: Partial<Schedule> = useMemo(() => ({
    name: name.trim(),
    prompt: prompt.trim(),
    cron_expression: mode === 'cron' ? cronValue : null,
    run_at: mode === 'once' ? localInputToIso(runAtLocal) : null,
    repos: selectedRepos.length > 0 ? selectedRepos.join(',') : null,
    skills: selectedSkills,
    mcp_servers: selectedMcp,
    max_open_tasks: maxOpenTasks,
    task_prefix: taskPrefix.trim() || null,
    priority,
    cli,
    model,
  }), [name, prompt, mode, cronValue, runAtLocal, selectedRepos, selectedSkills, selectedMcp, maxOpenTasks, taskPrefix, priority, cli, model]);

  const isDirty = useMemo(() => {
    const cur: Partial<Schedule> = {
      name: schedule.name,
      prompt: schedule.prompt,
      cron_expression: schedule.cron_expression,
      run_at: schedule.run_at,
      repos: schedule.repos,
      skills: schedule.skills ?? [],
      mcp_servers: schedule.mcp_servers ?? [],
      max_open_tasks: schedule.max_open_tasks,
      task_prefix: schedule.task_prefix,
      priority: schedule.priority,
      cli: schedule.cli,
      model: schedule.model,
    };
    const arrEq = (a: string[] | null | undefined, b: string[] | null | undefined) => {
      const aa = a ?? []; const bb = b ?? [];
      return aa.length === bb.length && aa.every((x, i) => x === bb[i]);
    };
    return (
      cur.name !== desiredUpdates.name ||
      cur.prompt !== desiredUpdates.prompt ||
      cur.cron_expression !== desiredUpdates.cron_expression ||
      cur.run_at !== desiredUpdates.run_at ||
      cur.repos !== desiredUpdates.repos ||
      !arrEq(cur.skills, desiredUpdates.skills) ||
      !arrEq(cur.mcp_servers, desiredUpdates.mcp_servers) ||
      cur.max_open_tasks !== desiredUpdates.max_open_tasks ||
      cur.task_prefix !== desiredUpdates.task_prefix ||
      cur.priority !== desiredUpdates.priority ||
      cur.cli !== desiredUpdates.cli ||
      cur.model !== desiredUpdates.model
    );
  }, [schedule, desiredUpdates]);

  const canSave =
    !!desiredUpdates.name && !!desiredUpdates.prompt &&
    (mode === 'cron' ? !!cronValue : !!desiredUpdates.run_at);

  const handleSave = async () => {
    if (!canSave) return;
    setSaving(true);
    try {
      await onUpdate(desiredUpdates);
      toast.success('Schedule updated');
    } catch {
      toast.error('Failed to save schedule');
    } finally {
      setSaving(false);
    }
  };

  const toggleItem = (list: string[], setList: (v: string[]) => void, item: string) => {
    setList(list.includes(item) ? list.filter(i => i !== item) : [...list, item]);
  };

  const inputClass = "w-full h-9 px-3 text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-11 placeholder:text-gray-7 focus:outline-none focus:border-gray-6";
  const labelClass = "block text-[12px] font-medium text-gray-8 mb-2 uppercase tracking-wider";

  return (
    <div className="space-y-5">
      {/* Name */}
      <div>
        <label className={labelClass}>Name</label>
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          className={inputClass}
        />
      </div>

      {/* Prompt */}
      <div>
        <label className={labelClass}>Prompt</label>
        <textarea
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          rows={4}
          className="w-full px-3 py-2.5 text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-11 placeholder:text-gray-7 focus:outline-none focus:border-gray-6 font-mono"
        />
      </div>

      {/* Mode toggle */}
      <div>
        <label className={labelClass}>Schedule type</label>
        <div className="flex gap-1.5">
          <button
            type="button"
            onClick={() => setMode('cron')}
            className={`flex-1 h-9 rounded-lg text-[13px] font-medium transition-colors border flex items-center justify-center gap-1.5 ${
              mode === 'cron'
                ? 'bg-gray-3 border-gray-5 text-gray-12'
                : 'bg-transparent border-gray-4 text-gray-8 hover:text-gray-11 hover:bg-gray-3'
            }`}
          >
            <Clock size={14} weight="bold" /> Recurring
          </button>
          <button
            type="button"
            onClick={() => setMode('once')}
            className={`flex-1 h-9 rounded-lg text-[13px] font-medium transition-colors border flex items-center justify-center gap-1.5 ${
              mode === 'once'
                ? 'bg-gray-3 border-gray-5 text-gray-12'
                : 'bg-transparent border-gray-4 text-gray-8 hover:text-gray-11 hover:bg-gray-3'
            }`}
          >
            <Timer size={14} weight="bold" /> One-shot
          </button>
        </div>
      </div>

      {mode === 'cron' ? (
        <div>
          <label className={labelClass}>Frequency</label>
          <select
            value={cronPreset}
            onChange={(e) => setCronPreset(e.target.value)}
            className={`${inputClass} cursor-pointer`}
          >
            {CRON_PRESETS.map((p) => (
              <option key={p.value} value={p.value}>{p.label}</option>
            ))}
          </select>
          {!cronPreset && (
            <input
              value={customCron}
              onChange={(e) => setCustomCron(e.target.value)}
              placeholder="e.g. 0 */6 * * *"
              className={`${inputClass} mt-2 font-mono`}
            />
          )}
          {cronValue && cronPreset && (
            <p className="text-[12px] text-gray-7 font-mono mt-1.5">{cronValue}</p>
          )}
        </div>
      ) : (
        <div>
          <label className={labelClass}>Run at</label>
          <input
            type="datetime-local"
            value={runAtLocal}
            onChange={(e) => setRunAtLocal(e.target.value)}
            className={inputClass}
          />
        </div>
      )}

      {/* Repos + Prefix + Max */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <div>
          <label className={labelClass}>Repositories</label>
          <div className="space-y-1.5 max-h-40 overflow-y-auto">
            {repos.length === 0 ? (
              <p className="text-[13px] text-gray-7">All configured repos</p>
            ) : (
              repos.map((r) => (
                <label key={r} className="flex items-center gap-2 cursor-pointer text-[13px] text-gray-11">
                  <input
                    type="checkbox"
                    checked={selectedRepos.includes(r)}
                    onChange={() => toggleItem(selectedRepos, setSelectedRepos, r)}
                    className="rounded border-gray-5"
                  />
                  {r}
                </label>
              ))
            )}
          </div>
          {selectedRepos.length === 0 && repos.length > 0 && (
            <p className="text-[11px] text-gray-7 mt-1">None selected = all repos</p>
          )}
        </div>
        <div className="space-y-4">
          <div>
            <label className={labelClass}>Task prefix</label>
            <input
              value={taskPrefix}
              onChange={(e) => setTaskPrefix(e.target.value.toUpperCase())}
              placeholder="e.g. BUG, FEA, SEC"
              className={`${inputClass} font-mono`}
            />
          </div>
          <div>
            <label className={labelClass}>Max open tasks</label>
            <input
              type="number"
              min={0}
              max={50}
              value={maxOpenTasks}
              onChange={(e) => setMaxOpenTasks(parseInt(e.target.value) || 0)}
              className={inputClass}
            />
            <p className="text-[11px] text-gray-7 mt-1">0 = unlimited</p>
          </div>
        </div>
      </div>

      {/* Priority */}
      <div>
        <label className={labelClass}>Task priority</label>
        <div className="flex gap-1.5">
          {PRIORITIES.map((p) => (
            <button
              key={p.value}
              type="button"
              onClick={() => setPriority(p.value)}
              className={`flex-1 h-9 rounded-lg text-[12px] font-medium transition-colors border ${
                priority === p.value
                  ? 'bg-gray-3 border-gray-5 text-gray-12'
                  : 'bg-transparent border-gray-4 text-gray-8 hover:text-gray-11 hover:bg-gray-3'
              }`}
            >
              {p.label}
            </button>
          ))}
        </div>
      </div>

      {/* Agent + Model */}
      {backendNames.length > 0 && (
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div>
            <label className={labelClass}>Agent CLI</label>
            <select value={cli} onChange={(e) => setCli(e.target.value)} className={`${inputClass} cursor-pointer`}>
              {backendNames.map((n) => <option key={n} value={n}>{n}</option>)}
            </select>
          </div>
          <div>
            <label className={labelClass}>Model</label>
            <input
              list={`schedule-edit-models-${cli}`}
              value={model}
              onChange={(e) => setModel(e.target.value)}
              placeholder="model name (any)"
              className={inputClass}
            />
            <datalist id={`schedule-edit-models-${cli}`}>
              {currentModels.map((m) => <option key={m} value={m} />)}
            </datalist>
          </div>
        </div>
      )}

      {/* Skills */}
      {allSkills.length > 0 && (
        <div>
          <label className={labelClass}>Skills</label>
          <div className="flex flex-wrap gap-1.5">
            {allSkills.map((s) => (
              <button
                key={s}
                type="button"
                onClick={() => toggleItem(selectedSkills, setSelectedSkills, s)}
                className={`px-2.5 h-7 rounded-md text-[12px] font-medium transition-colors border ${
                  selectedSkills.includes(s)
                    ? 'bg-gray-3 border-gray-5 text-gray-12'
                    : 'bg-transparent border-gray-4 text-gray-8 hover:text-gray-11'
                }`}
              >
                {s}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* MCP servers */}
      {allMcpServers.length > 0 && (
        <div>
          <label className={labelClass}>MCP servers</label>
          <div className="flex flex-wrap gap-1.5">
            {allMcpServers.map((s) => (
              <button
                key={s}
                type="button"
                onClick={() => toggleItem(selectedMcp, setSelectedMcp, s)}
                className={`px-2.5 h-7 rounded-md text-[12px] font-medium transition-colors border ${
                  selectedMcp.includes(s)
                    ? 'bg-gray-3 border-gray-5 text-gray-12'
                    : 'bg-transparent border-gray-4 text-gray-8 hover:text-gray-11'
                }`}
              >
                {s}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Save changes */}
      <div className="flex items-center justify-end gap-2 pt-4 border-t border-gray-3">
        <button
          onClick={handleSave}
          disabled={!isDirty || !canSave || saving}
          className="h-9 px-4 text-[14px] font-medium text-white bg-sun-9 hover:bg-sun-10 text-gray-1 rounded-lg transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {saving ? 'Saving...' : 'Save changes'}
        </button>
      </div>

      {/* Enable / pause */}
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
        {schedule.cron_expression && (
          <div className="flex items-center gap-1.5 font-mono text-[12px] text-gray-7">
            <Gear size={13} weight="bold" />
            {humanizeCron(schedule.cron_expression)}
          </div>
        )}
      </div>
    </div>
  );
}
