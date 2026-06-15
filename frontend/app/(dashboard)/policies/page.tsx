'use client';

import { useState } from 'react';
import { useSession } from 'next-auth/react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import toast from 'react-hot-toast';
import { useAuthDisabled } from '@/lib/auth-context';
import {
  Policy,
  PolicySeverity,
  fetchPolicies,
  createPolicy,
  updatePolicy,
  deletePolicy,
} from '@/lib/policies';
import { Plus, Trash, ShieldCheck, Warning } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Card } from '@/components/ui/card';
import { PageHeader } from '@/components/ui/page-header';

const POLICIES_KEY = ['policies'] as const;

export default function PoliciesPage() {
  const authDisabled = useAuthDisabled();
  const { status: authStatus } = useSession();
  const isReady = authDisabled || authStatus === 'authenticated';
  const qc = useQueryClient();

  const { data: policies = [], isLoading } = useQuery<Policy[]>({
    queryKey: POLICIES_KEY,
    queryFn: ({ signal }) => fetchPolicies(signal),
    enabled: isReady,
    refetchInterval: 10_000,
  });

  const [showCreate, setShowCreate] = useState(false);

  const createMutation = useMutation({
    mutationFn: createPolicy,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: POLICIES_KEY });
      setShowCreate(false);
      toast.success('Policy created');
    },
    onError: () => toast.error('Failed to create policy'),
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<Policy> }) => updatePolicy(id, data),
    onMutate: async ({ id, data }) => {
      await qc.cancelQueries({ queryKey: POLICIES_KEY });
      const prev = qc.getQueryData<Policy[]>(POLICIES_KEY);
      if (prev) qc.setQueryData(POLICIES_KEY, prev.map(p => p.id === id ? { ...p, ...data } : p));
      return { prev };
    },
    onError: (_e, _v, ctx) => {
      if (ctx?.prev) qc.setQueryData(POLICIES_KEY, ctx.prev);
      toast.error('Failed to update policy');
    },
    onSettled: () => qc.invalidateQueries({ queryKey: POLICIES_KEY }),
  });

  const deleteMutation = useMutation({
    mutationFn: deletePolicy,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: POLICIES_KEY });
      toast.success('Policy deleted');
    },
    onError: () => toast.error('Failed to delete policy'),
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="w-5 h-5 border-2 border-gray-7 border-t-gray-12 rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto">
      <div className="w-full px-4 sm:px-6 lg:px-8 py-6">
        <PageHeader
          title="Policies"
          description="Advisory guardrails surfaced on the plan review tab. Matched policies become a banner on the task."
          actions={
            <Button variant="primary" size="md" onClick={() => setShowCreate(true)}>
              <Plus size={15} weight="bold" />
              <span className="hidden sm:inline">New policy</span>
            </Button>
          }
        />

        {showCreate && (
          <CreatePolicyForm
            onSubmit={(data) => createMutation.mutate(data)}
            onCancel={() => setShowCreate(false)}
            submitting={createMutation.isPending}
          />
        )}

        {policies.length === 0 && !showCreate ? (
          <div className="flex flex-col items-center justify-center py-20 text-gray-8">
            <ShieldCheck size={48} weight="thin" className="mb-4" />
            <p className="text-[15px] font-medium text-gray-10">No policies yet</p>
            <p className="text-[13px] mt-1.5 max-w-md text-center">
              Add a policy to flag risky paths during plan review. Example: warn whenever a plan touches
              <code className="font-mono text-gray-9 mx-1">migrations/**</code>.
            </p>
          </div>
        ) : (
          <div className="space-y-2">
            {policies.map((p) => (
              <PolicyRow
                key={p.id}
                policy={p}
                onToggle={(enabled) => updateMutation.mutate({ id: p.id, data: { enabled } })}
                onDelete={() => {
                  if (confirm(`Delete policy "${p.name}"?`)) deleteMutation.mutate(p.id);
                }}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function PolicyRow({ policy, onToggle, onDelete }: {
  policy: Policy;
  onToggle: (enabled: boolean) => void;
  onDelete: () => void;
}) {
  return (
    <Card className="px-4 py-3 flex items-start gap-3">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 mb-1">
          <span className="text-[14px] font-medium text-gray-12 truncate tracking-[-0.01em]">{policy.name}</span>
          <Badge tone={policy.severity === 'block' ? 'danger' : 'warning'}>
            <Warning size={10} weight="bold" /> {policy.severity}
          </Badge>
          {!policy.enabled && <Badge tone="neutral">disabled</Badge>}
        </div>
        <div className="text-[12px] text-gray-9 mb-1">{policy.message}</div>
        <div className="flex items-center gap-2 text-[11px] text-gray-7 font-mono">
          <span>repo:</span><span className="text-gray-9">{policy.repo_pattern}</span>
          <span>·</span>
          <span>path:</span><span className="text-gray-9">{policy.path_glob}</span>
        </div>
      </div>
      <div className="flex items-center gap-1 flex-shrink-0">
        <button
          onClick={() => onToggle(!policy.enabled)}
          className={`relative w-9 h-5 rounded-full transition-smooth focus-ring ${
            policy.enabled ? 'bg-sun-9' : 'bg-gray-5'
          }`}
          title={policy.enabled ? 'Disable' : 'Enable'}
        >
          <span
            className={`absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full transition-transform ${
              policy.enabled ? 'translate-x-4' : 'translate-x-0'
            }`}
          />
        </button>
        <button
          onClick={onDelete}
          className="h-7 w-7 flex items-center justify-center text-gray-8 hover:text-red-400 hover:bg-gray-3 rounded-md transition-smooth focus-ring"
          title="Delete"
        >
          <Trash size={14} weight="bold" />
        </button>
      </div>
    </Card>
  );
}

function CreatePolicyForm({ onSubmit, onCancel, submitting }: {
  onSubmit: (data: { name: string; repo_pattern: string; path_glob: string; message: string; severity: PolicySeverity }) => void;
  onCancel: () => void;
  submitting: boolean;
}) {
  const [name, setName] = useState('');
  const [repoPattern, setRepoPattern] = useState('*');
  const [pathGlob, setPathGlob] = useState('');
  const [message, setMessage] = useState('');
  const [severity, setSeverity] = useState<PolicySeverity>('warn');

  const inputClass = "w-full h-9 px-3 text-[13px] rounded-lg bg-gray-3 border border-gray-4 text-gray-12 placeholder:text-gray-7 transition-smooth focus-ring focus-visible:border-gray-6";

  return (
    <Card className="p-4 mb-3 space-y-3 animate-fade-in-up">
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <div>
          <label className="block text-[11px] font-medium text-gray-8 uppercase tracking-wider mb-1.5">Name</label>
          <input className={inputClass} value={name} onChange={(e) => setName(e.target.value)} placeholder="Migrations need a rollback plan" />
        </div>
        <div>
          <label className="block text-[11px] font-medium text-gray-8 uppercase tracking-wider mb-1.5">Severity</label>
          <select className={inputClass} value={severity} onChange={(e) => setSeverity(e.target.value as PolicySeverity)}>
            <option value="warn">warn (visual banner)</option>
            <option value="block">block (reserved)</option>
          </select>
        </div>
      </div>
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <div>
          <label className="block text-[11px] font-medium text-gray-8 uppercase tracking-wider mb-1.5">Repo pattern</label>
          <input className={inputClass + ' font-mono'} value={repoPattern} onChange={(e) => setRepoPattern(e.target.value)} placeholder="*  or owner/repo  or owner/*" />
        </div>
        <div>
          <label className="block text-[11px] font-medium text-gray-8 uppercase tracking-wider mb-1.5">Path glob</label>
          <input className={inputClass + ' font-mono'} value={pathGlob} onChange={(e) => setPathGlob(e.target.value)} placeholder="migrations/**" />
        </div>
      </div>
      <div>
        <label className="block text-[11px] font-medium text-gray-8 uppercase tracking-wider mb-1.5">Message</label>
        <input className={inputClass} value={message} onChange={(e) => setMessage(e.target.value)} placeholder="Schema change - verify rollback plan." />
      </div>
      <div className="flex justify-end gap-2 pt-1">
        <Button type="button" variant="ghost" size="md" onClick={onCancel}>
          Cancel
        </Button>
        <Button
          type="button"
          variant="primary"
          size="md"
          disabled={submitting || !name.trim() || !pathGlob.trim() || !message.trim()}
          onClick={() => onSubmit({ name: name.trim(), repo_pattern: repoPattern.trim() || '*', path_glob: pathGlob.trim(), message: message.trim(), severity })}
        >
          {submitting ? 'Creating...' : 'Create'}
        </Button>
      </div>
    </Card>
  );
}
