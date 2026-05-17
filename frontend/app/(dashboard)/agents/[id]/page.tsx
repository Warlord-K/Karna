'use client';

import { use, useState } from 'react';
import { useRouter } from 'next/navigation';
import Link from 'next/link';
import { useSession } from 'next-auth/react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import toast from 'react-hot-toast';
import { useAuthDisabled } from '@/lib/auth-context';
import { useConfig } from '@/hooks/use-tasks';
import { AgentProfile, updateAgent } from '@/lib/agents';
import { AgentTask, getTaskLabel, getTaskTitle } from '@/lib/agent-tasks';
import { ArrowLeft, Robot, Lightning, GitPullRequest } from '@phosphor-icons/react';

interface AgentStats {
  total_tasks: number;
  open_tasks: number;
  prs_opened: number;
  reviews_done: number;
  cost_usd: number;
}

interface PrReview {
  id: string;
  repo: string;
  pr_number: number;
  pr_url: string | null;
  head_sha: string;
  status: string;
  cost_usd: number;
  created_at: string | null;
  completed_at: string | null;
}

export default function AgentDetailPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = use(params);
  const router = useRouter();
  const authDisabled = useAuthDisabled();
  const { status: authStatus } = useSession();
  const isReady = authDisabled || authStatus === 'authenticated';
  const qc = useQueryClient();
  const { data: config } = useConfig(isReady);
  const backends = config?.backends ?? {};

  const { data: profile, isLoading } = useQuery<AgentProfile>({
    queryKey: ['agents', id],
    queryFn: async () => {
      const res = await fetch(`/api/agents/${id}`);
      if (!res.ok) throw new Error('Failed to load agent');
      return res.json();
    },
    enabled: isReady,
  });

  const { data: stats } = useQuery<AgentStats>({
    queryKey: ['agents', id, 'stats'],
    queryFn: async () => {
      const res = await fetch(`/api/agents/${id}/stats`);
      if (!res.ok) throw new Error('Failed to load stats');
      return res.json();
    },
    enabled: isReady,
    refetchInterval: 10_000,
  });

  const { data: recentTasks = [] } = useQuery<AgentTask[]>({
    queryKey: ['agents', id, 'tasks'],
    queryFn: async () => {
      const res = await fetch(`/api/agents/${id}/tasks`);
      if (!res.ok) throw new Error('Failed to load tasks');
      return res.json();
    },
    enabled: isReady,
    refetchInterval: 10_000,
  });

  const { data: reviews = [] } = useQuery<PrReview[]>({
    queryKey: ['agents', id, 'reviews'],
    queryFn: async () => {
      const res = await fetch(`/api/agents/${id}/reviews`);
      if (!res.ok) throw new Error('Failed to load reviews');
      return res.json();
    },
    enabled: isReady,
  });

  const [editing, setEditing] = useState(false);
  const [editName, setEditName] = useState('');
  const [editEmoji, setEditEmoji] = useState('');
  const [editCli, setEditCli] = useState('');
  const [editModel, setEditModel] = useState('');
  const [editAddendum, setEditAddendum] = useState('');
  const [paused, setPaused] = useState<string | null>(null);

  const startEdit = () => {
    if (!profile) return;
    setEditName(profile.name);
    setEditEmoji(profile.avatar_emoji);
    setEditCli(profile.cli);
    setEditModel(profile.model);
    setEditAddendum(profile.system_prompt_addendum ?? '');
    setPaused(profile.paused_reason);
    setEditing(true);
  };

  const save = async () => {
    if (!profile) return;
    try {
      await updateAgent(profile.id, {
        name: editName,
        avatar_emoji: editEmoji,
        cli: editCli,
        model: editModel,
        system_prompt_addendum: editAddendum || null,
        paused_reason: paused || null,
      } as Partial<AgentProfile>);
      qc.invalidateQueries({ queryKey: ['agents'] });
      toast.success('Agent updated');
      setEditing(false);
    } catch {
      toast.error('Failed to save');
    }
  };

  const togglePause = async () => {
    if (!profile) return;
    const reason = profile.paused_reason ? null : 'manually paused';
    try {
      await updateAgent(profile.id, { paused_reason: reason } as Partial<AgentProfile>);
      qc.invalidateQueries({ queryKey: ['agents'] });
      toast.success(reason ? 'Agent paused' : 'Agent resumed');
    } catch {
      toast.error('Failed to toggle pause');
    }
  };

  const makeDefault = async () => {
    if (!profile) return;
    try {
      await updateAgent(profile.id, { is_default: true } as Partial<AgentProfile>);
      qc.invalidateQueries({ queryKey: ['agents'] });
      toast.success('Set as default agent');
    } catch {
      toast.error('Failed to set default');
    }
  };

  if (isLoading || !profile) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="w-5 h-5 border-2 border-gray-7 border-t-gray-12 rounded-full animate-spin" />
      </div>
    );
  }

  const inputClass = "w-full h-9 px-3 text-[13px] rounded-lg bg-gray-3 border border-gray-4 text-gray-12 placeholder:text-gray-7 focus:outline-none focus:border-gray-6";
  const labelClass = "block text-[11px] font-medium text-gray-8 uppercase tracking-wider mb-1.5";
  const availableModels = backends[editCli]?.models ?? [profile.model];
  const backendNames = Object.keys(backends);

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-3xl mx-auto px-4 sm:px-6 py-6">
        <button
          onClick={() => router.push('/agents')}
          className="h-8 px-2 text-[12px] text-gray-8 hover:text-gray-12 hover:bg-gray-3 rounded-md transition-colors flex items-center gap-1.5 mb-3"
        >
          <ArrowLeft size={14} weight="bold" /> All agents
        </button>

        <div className="flex items-start justify-between gap-4 mb-6">
          <div className="flex items-center gap-3 min-w-0">
            <span className="text-[36px]" aria-hidden>{profile.avatar_emoji}</span>
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <h1 className="text-[22px] font-semibold text-gray-12 tracking-[-0.02em] truncate">{profile.name}</h1>
                {profile.is_default && (
                  <span className="inline-flex items-center gap-1 px-1.5 h-5 rounded text-[10px] bg-sun-9/15 border border-sun-9/30 text-sun-10">
                    <Lightning size={10} weight="bold" /> default
                  </span>
                )}
                {profile.paused_reason && (
                  <span
                    title={profile.paused_reason}
                    className="inline-flex items-center px-1.5 h-5 rounded text-[10px] bg-amber-500/15 border border-amber-500/30 text-amber-400"
                  >
                    paused
                  </span>
                )}
              </div>
              <div className="text-[12px] text-gray-7 font-mono mt-1">
                {profile.cli} · {profile.model} · <span className="text-gray-9">{profile.slug}</span>
              </div>
            </div>
          </div>
          <div className="flex items-center gap-2 flex-shrink-0">
            {!profile.is_default && (
              <button
                onClick={makeDefault}
                className="h-8 px-3 text-[12px] text-gray-9 hover:text-gray-12 hover:bg-gray-3 rounded-md transition-colors"
              >
                Set default
              </button>
            )}
            <button
              onClick={togglePause}
              className="h-8 px-3 text-[12px] text-gray-9 hover:text-gray-12 hover:bg-gray-3 rounded-md transition-colors"
            >
              {profile.paused_reason ? 'Resume' : 'Pause'}
            </button>
            {!editing && (
              <button
                onClick={startEdit}
                className="h-8 px-3 text-[12px] font-medium text-white bg-sun-9 hover:bg-sun-10 rounded-md transition-colors"
              >
                Edit
              </button>
            )}
          </div>
        </div>

        {/* Stats */}
        <div className="grid grid-cols-2 sm:grid-cols-5 gap-3 mb-6">
          <Stat label="Tasks" value={stats?.total_tasks ?? 0} />
          <Stat label="Open" value={stats?.open_tasks ?? 0} />
          <Stat label="PRs opened" value={stats?.prs_opened ?? 0} />
          <Stat label="Reviews" value={stats?.reviews_done ?? 0} />
          <Stat label="Cost" value={`$${(stats?.cost_usd ?? 0).toFixed(2)}`} />
        </div>

        {/* Edit form */}
        {editing && (
          <div className="bg-gray-2 border border-gray-3 rounded-lg p-4 space-y-3 mb-6">
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
              <div className="sm:col-span-2">
                <label className={labelClass}>Name</label>
                <input className={inputClass} value={editName} onChange={(e) => setEditName(e.target.value)} />
              </div>
              <div>
                <label className={labelClass}>Emoji</label>
                <input className={inputClass} value={editEmoji} onChange={(e) => setEditEmoji(e.target.value)} maxLength={4} />
              </div>
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <div>
                <label className={labelClass}>CLI</label>
                <select className={inputClass} value={editCli} onChange={(e) => setEditCli(e.target.value)}>
                  {backendNames.length === 0 && <option value={editCli}>{editCli}</option>}
                  {backendNames.map((n) => <option key={n} value={n}>{n}</option>)}
                </select>
              </div>
              <div>
                <label className={labelClass}>Model</label>
                <select className={inputClass} value={editModel} onChange={(e) => setEditModel(e.target.value)}>
                  {availableModels.map((m) => <option key={m} value={m}>{m}</option>)}
                </select>
              </div>
            </div>
            <div>
              <label className={labelClass}>System prompt addendum</label>
              <textarea
                className={inputClass.replace('h-9', 'min-h-[80px] py-2')}
                value={editAddendum}
                onChange={(e) => setEditAddendum(e.target.value)}
                placeholder="Extra instructions appended to this agent's CLI system prompt (e.g. 'always favor minimal diffs')"
              />
            </div>
            <div className="flex justify-end gap-2 pt-1">
              <button onClick={() => setEditing(false)} className="h-8 px-3 text-[13px] text-gray-9 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors">
                Cancel
              </button>
              <button onClick={save} className="h-8 px-4 text-[13px] font-medium text-white bg-sun-9 hover:bg-sun-10 rounded-lg transition-colors">
                Save
              </button>
            </div>
          </div>
        )}

        {/* Recent tasks */}
        <section className="mb-6">
          <h3 className="text-[14px] font-medium text-gray-11 mb-2">Recent tasks</h3>
          {recentTasks.length === 0 ? (
            <p className="text-[12px] text-gray-7">No tasks assigned to this agent yet.</p>
          ) : (
            <div className="space-y-1.5">
              {recentTasks.slice(0, 10).map((t) => (
                <Link
                  key={t.id}
                  href={`/tasks/${t.id}`}
                  className="block bg-gray-2 border border-gray-3 rounded-lg px-3 py-2 hover:bg-gray-3 transition-colors"
                >
                  <div className="flex items-center gap-2">
                    <span className="text-[12px] text-gray-7 font-mono">{getTaskLabel(t)}</span>
                    <span className="text-[13px] text-gray-12 truncate">{getTaskTitle(t)}</span>
                    <span className="text-[11px] text-gray-7 ml-auto flex-shrink-0">{t.status}</span>
                    {t.pr_url && <GitPullRequest size={12} weight="bold" className="text-gray-7" />}
                  </div>
                </Link>
              ))}
            </div>
          )}
        </section>

        {/* Recent PR reviews */}
        <section>
          <h3 className="text-[14px] font-medium text-gray-11 mb-2">Recent PR reviews</h3>
          {reviews.length === 0 ? (
            <p className="text-[12px] text-gray-7">This agent hasn&apos;t reviewed any PRs yet.</p>
          ) : (
            <div className="space-y-1.5">
              {reviews.slice(0, 10).map((r) => (
                <a
                  key={r.id}
                  href={r.pr_url ?? '#'}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="block bg-gray-2 border border-gray-3 rounded-lg px-3 py-2 hover:bg-gray-3 transition-colors"
                >
                  <div className="flex items-center gap-2">
                    <span className="text-[12px] text-gray-9 font-mono">{r.repo}#{r.pr_number}</span>
                    <span className="text-[11px] text-gray-7 ml-auto flex-shrink-0">{r.status}</span>
                    {r.cost_usd > 0 && <span className="text-[11px] text-gray-7 font-mono">${r.cost_usd.toFixed(3)}</span>}
                  </div>
                </a>
              ))}
            </div>
          )}
        </section>
      </div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="bg-gray-2 border border-gray-3 rounded-lg px-3 py-2">
      <div className="text-[10px] text-gray-7 uppercase tracking-wider">{label}</div>
      <div className="text-[18px] font-semibold text-gray-12 tabular-nums mt-0.5">{value}</div>
    </div>
  );
}
