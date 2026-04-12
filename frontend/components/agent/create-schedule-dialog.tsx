'use client';

import { useState, useEffect } from 'react';
import { SchedulePriority } from '@/lib/schedules';
import { X, Clock, Timer } from '@phosphor-icons/react';
import { BackendConfig } from './create-task-dialog';

interface CreateScheduleDialogProps {
  open: boolean;
  onClose: () => void;
  repos: string[];
  backends: Record<string, BackendConfig>;
  skills: string[];
  mcpServers: string[];
  onCreateSchedule: (data: {
    name: string;
    prompt: string;
    repos: string | null;
    cron_expression: string | null;
    run_at: string | null;
    skills: string[];
    mcp_servers: string[];
    max_open_tasks: number;
    task_prefix: string | null;
    priority: SchedulePriority;
    cli: string | null;
    model: string | null;
  }) => Promise<void>;
}

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

export function CreateScheduleDialog({
  open, onClose, repos, backends, skills, mcpServers, onCreateSchedule,
}: CreateScheduleDialogProps) {
  const backendNames = Object.keys(backends);
  const defaultCli = backendNames[0] || 'claude';
  const defaultModel = backends[defaultCli]?.default_model || '';

  const [name, setName] = useState('');
  const [prompt, setPrompt] = useState('');
  const [selectedRepos, setSelectedRepos] = useState<string[]>([]);
  const [mode, setMode] = useState<'cron' | 'once'>('cron');
  const [cronPreset, setCronPreset] = useState('0 */4 * * *');
  const [customCron, setCustomCron] = useState('');
  const [runAt, setRunAt] = useState('');
  const [selectedSkills, setSelectedSkills] = useState<string[]>([]);
  const [selectedMcp, setSelectedMcp] = useState<string[]>([]);
  const [maxOpenTasks, setMaxOpenTasks] = useState(3);
  const [taskPrefix, setTaskPrefix] = useState('');
  const [priority, setPriority] = useState<SchedulePriority>('medium');
  const [cli, setCli] = useState(defaultCli);
  const [model, setModel] = useState(defaultModel);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const backend = backends[cli];
    if (backend) setModel(backend.default_model || backend.models[0] || '');
  }, [cli, backends]);

  if (!open) return null;

  const currentModels = backends[cli]?.models || [];
  const cronValue = cronPreset || customCron;
  const selectClass = "w-full h-9 px-3 text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-11 focus:outline-none focus:border-gray-6 cursor-pointer";
  const labelClass = "block text-[12px] font-medium text-gray-8 mb-2 uppercase tracking-wider";

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !prompt.trim()) return;
    setLoading(true);
    try {
      await onCreateSchedule({
        name: name.trim(),
        prompt: prompt.trim(),
        repos: selectedRepos.length > 0 ? selectedRepos.join(',') : null,
        cron_expression: mode === 'cron' ? cronValue : null,
        run_at: mode === 'once' ? runAt || null : null,
        skills: selectedSkills,
        mcp_servers: selectedMcp,
        max_open_tasks: maxOpenTasks,
        task_prefix: taskPrefix.trim() || null,
        priority,
        cli,
        model,
      });
      // Reset form
      setName(''); setPrompt(''); setSelectedRepos([]); setCronPreset('0 */4 * * *');
      setCustomCron(''); setRunAt(''); setSelectedSkills([]); setSelectedMcp([]);
      setMaxOpenTasks(3); setTaskPrefix(''); setPriority('medium');
      setCli(defaultCli); setModel(defaultModel); setMode('cron');
      onClose();
    } catch (error) {
      console.error(error);
    } finally {
      setLoading(false);
    }
  };

  const toggleItem = (list: string[], setList: (v: string[]) => void, item: string) => {
    setList(list.includes(item) ? list.filter(i => i !== item) : [...list, item]);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-end sm:items-center justify-center">
      <div className="fixed inset-0 bg-black/50" onClick={onClose} />
      <div className="relative bg-gray-1 rounded-t-2xl sm:rounded-xl shadow-modal w-full sm:max-w-[640px] sm:mx-6 max-h-[90vh] sm:max-h-[85vh] overflow-y-auto">
        <div className="sm:hidden flex justify-center pt-2 pb-0">
          <div className="w-8 h-1 rounded-full bg-gray-6" />
        </div>

        <div className="flex items-center justify-between px-4 sm:px-6 h-13 border-b border-gray-3">
          <span className="text-[15px] font-semibold text-gray-12 tracking-[-0.01em]">New schedule</span>
          <button onClick={onClose} className="h-8 w-8 sm:h-7 sm:w-7 flex items-center justify-center text-gray-8 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors">
            <X size={16} weight="bold" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-4 sm:p-6 space-y-4 sm:space-y-5">
          {/* Name */}
          <div>
            <label className={labelClass}>Name</label>
            <input
              placeholder="e.g. Bug Hunter, Security Audit"
              value={name}
              onChange={(e) => setName(e.target.value)}
              autoFocus
              className="w-full h-10 sm:h-9 px-3 text-[16px] sm:text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-12 placeholder:text-gray-7 focus:outline-none focus:border-gray-6"
            />
          </div>

          {/* Prompt */}
          <div>
            <label className={labelClass}>Prompt</label>
            <textarea
              placeholder="What should the agent look for or do?"
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              rows={3}
              className="w-full px-3 py-2.5 text-[16px] sm:text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-11 placeholder:text-gray-7 focus:outline-none focus:border-gray-6 font-mono"
            />
          </div>

          {/* Mode toggle: Recurring vs One-shot */}
          <div>
            <label className={labelClass}>Schedule type</label>
            <div className="flex gap-1.5">
              <button
                type="button"
                onClick={() => setMode('cron')}
                className={`flex-1 h-10 sm:h-9 rounded-lg text-[13px] font-medium transition-colors border flex items-center justify-center gap-1.5 ${
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
                className={`flex-1 h-10 sm:h-9 rounded-lg text-[13px] font-medium transition-colors border flex items-center justify-center gap-1.5 ${
                  mode === 'once'
                    ? 'bg-gray-3 border-gray-5 text-gray-12'
                    : 'bg-transparent border-gray-4 text-gray-8 hover:text-gray-11 hover:bg-gray-3'
                }`}
              >
                <Timer size={14} weight="bold" /> One-shot
              </button>
            </div>
          </div>

          {/* Cron or run_at */}
          {mode === 'cron' ? (
            <div>
              <label className={labelClass}>Frequency</label>
              <select
                value={cronPreset}
                onChange={(e) => setCronPreset(e.target.value)}
                className={selectClass}
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
                  className="w-full h-9 px-3 mt-2 text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-11 placeholder:text-gray-7 focus:outline-none focus:border-gray-6 font-mono"
                />
              )}
            </div>
          ) : (
            <div>
              <label className={labelClass}>Run at</label>
              <input
                type="datetime-local"
                value={runAt}
                onChange={(e) => setRunAt(e.target.value)}
                className="w-full h-9 px-3 text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-11 focus:outline-none focus:border-gray-6"
              />
            </div>
          )}

          {/* Repos + Task prefix */}
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div>
              <label className={labelClass}>Repositories</label>
              <div className="space-y-1.5 max-h-32 overflow-y-auto">
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
                      {r.split('/').pop()}
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
                  className="w-full h-9 px-3 text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-11 placeholder:text-gray-7 focus:outline-none focus:border-gray-6 font-mono"
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
                  className="w-full h-9 px-3 text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-11 focus:outline-none focus:border-gray-6"
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
                  className={`flex-1 h-10 sm:h-9 rounded-lg text-[12px] font-medium transition-colors border ${
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
                <label className={labelClass}>Agent</label>
                <select value={cli} onChange={(e) => setCli(e.target.value)} className={selectClass}>
                  {backendNames.map((n) => <option key={n} value={n}>{n}</option>)}
                </select>
              </div>
              <div>
                <label className={labelClass}>Model</label>
                <select value={model} onChange={(e) => setModel(e.target.value)} className={selectClass}>
                  {currentModels.map((m) => <option key={m} value={m}>{m}</option>)}
                </select>
              </div>
            </div>
          )}

          {/* Skills */}
          {skills.length > 0 && (
            <div>
              <label className={labelClass}>Skills</label>
              <div className="flex flex-wrap gap-1.5">
                {skills.map((s) => (
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

          {/* MCP Servers */}
          {mcpServers.length > 0 && (
            <div>
              <label className={labelClass}>MCP servers</label>
              <div className="flex flex-wrap gap-1.5">
                {mcpServers.map((s) => (
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

          {/* Actions */}
          <div className="flex justify-end gap-2.5 pt-4 border-t border-gray-3 pb-safe">
            <button
              type="button"
              onClick={onClose}
              className="h-10 sm:h-9 px-4 text-[14px] text-gray-9 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={loading || !name.trim() || !prompt.trim() || (mode === 'cron' && !cronValue) || (mode === 'once' && !runAt)}
              className="h-10 sm:h-9 px-4 text-[14px] font-medium text-white bg-sun-9 hover:bg-sun-10 text-gray-1 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {loading ? 'Creating...' : 'Create schedule'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
