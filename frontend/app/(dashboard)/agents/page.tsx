'use client';

import { useState, useEffect } from 'react';
import { useSession } from 'next-auth/react';
import Link from 'next/link';
import { useQueryClient } from '@tanstack/react-query';
import toast from 'react-hot-toast';
import { useAuthDisabled } from '@/lib/auth-context';
import { useAgents, useConfig } from '@/hooks/use-tasks';
import { createAgent } from '@/lib/agents';
import { Robot, Lightning, Plus, X } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { PageHeader } from '@/components/ui/page-header';

function slugify(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

export default function AgentsIndexPage() {
  const authDisabled = useAuthDisabled();
  const { status: authStatus } = useSession();
  const isReady = authDisabled || authStatus === 'authenticated';
  const { data: agents = [], isLoading } = useAgents(isReady);
  const { data: config } = useConfig(isReady);
  const qc = useQueryClient();

  const [createOpen, setCreateOpen] = useState(false);

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
          title="Agents"
          description={
            <>
              Named agent identities. The ones from <code className="font-mono text-gray-9">config.yaml</code> are auto-seeded; create custom personas here.
            </>
          }
          actions={
            <Button variant="primary" size="md" onClick={() => setCreateOpen(true)}>
              <Plus size={14} weight="bold" /> New agent
            </Button>
          }
        />

        {agents.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-20 text-gray-8">
            <Robot size={48} weight="thin" className="mb-4" />
            <p className="text-[15px] font-medium text-gray-10">No agent profiles yet</p>
            <p className="text-[13px] mt-1.5 max-w-md text-center">
              The agent worker seeds profiles from your config.yaml backends on startup, or create one manually.
            </p>
          </div>
        ) : (
          <div className="space-y-1.5">
            {agents.map((a) => (
              <Card key={a.id} interactive className="px-4 py-3">
                <Link href={`/agents/${a.id}`} className="block focus-ring rounded-xl">
                  <div className="flex items-center gap-3">
                    <span className="text-[22px]" aria-hidden>{a.avatar_emoji}</span>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-[14px] font-medium text-gray-12 tracking-[-0.01em]">{a.name}</span>
                        {a.is_default && (
                          <Badge tone="accent">
                            <Lightning size={10} weight="bold" /> default
                          </Badge>
                        )}
                        {a.paused_reason && <Badge tone="warning">paused</Badge>}
                      </div>
                      <div className="text-[11px] text-gray-7 font-mono mt-0.5">
                        {a.cli} · {a.model}
                      </div>
                    </div>
                  </div>
                </Link>
              </Card>
            ))}
          </div>
        )}
      </div>

      <NewAgentDialog
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        backends={config?.backends ?? {}}
        onCreated={() => qc.invalidateQueries({ queryKey: ['agents'] })}
      />
    </div>
  );
}

function NewAgentDialog({
  open, onClose, backends, onCreated,
}: {
  open: boolean;
  onClose: () => void;
  backends: Record<string, { models: string[]; default_model: string }>;
  onCreated: () => void;
}) {
  const backendNames = Object.keys(backends);
  const defaultCli = backendNames[0] || 'claude';

  const [name, setName] = useState('');
  const [slug, setSlug] = useState('');
  const [slugDirty, setSlugDirty] = useState(false);
  const [emoji, setEmoji] = useState('🤖');
  const [cli, setCli] = useState(defaultCli);
  const [model, setModel] = useState(backends[defaultCli]?.default_model ?? '');
  const [addendum, setAddendum] = useState('');
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!slugDirty) setSlug(slugify(name));
  }, [name, slugDirty]);

  useEffect(() => {
    const b = backends[cli];
    if (b) setModel(b.default_model || b.models[0] || '');
  }, [cli, backends]);

  if (!open) return null;

  const currentModels = backends[cli]?.models ?? [];
  const canSubmit = !!name.trim() && !!slug.trim() && !!cli && !!model && !saving;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!canSubmit) return;
    setSaving(true);
    try {
      await createAgent({
        slug: slug.trim(),
        name: name.trim(),
        cli: cli.trim(),
        model: model.trim(),
        avatar_emoji: emoji.trim() || '🤖',
        system_prompt_addendum: addendum.trim() || undefined,
      });
      toast.success('Agent created');
      onCreated();
      // Reset
      setName(''); setSlug(''); setSlugDirty(false); setEmoji('🤖');
      setCli(defaultCli); setModel(backends[defaultCli]?.default_model ?? ''); setAddendum('');
      onClose();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to create agent');
    } finally {
      setSaving(false);
    }
  };

  const inputClass = "w-full h-9 px-3 text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-11 placeholder:text-gray-7 transition-smooth focus-ring focus-visible:border-gray-6";
  const labelClass = "block text-[12px] font-medium text-gray-8 mb-1.5 uppercase tracking-wider";

  return (
    <div className="fixed inset-0 z-50 flex items-end sm:items-center justify-center">
      <div className="fixed inset-0 bg-black/50" onClick={onClose} />
      <div className="relative bg-gray-1 rounded-t-2xl sm:rounded-xl shadow-modal w-full sm:max-w-[560px] sm:mx-6 max-h-[90vh] overflow-y-auto">
        <div className="sm:hidden flex justify-center pt-2 pb-0">
          <div className="w-8 h-1 rounded-full bg-gray-6" />
        </div>

        <div className="flex items-center justify-between px-4 sm:px-6 h-13 border-b border-gray-3">
          <span className="text-[15px] font-semibold text-gray-12 tracking-[-0.01em]">New agent</span>
          <button onClick={onClose} className="h-8 w-8 flex items-center justify-center text-gray-8 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors">
            <X size={16} weight="bold" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-4 sm:p-6 space-y-4">
          <div className="grid grid-cols-[1fr_auto] gap-3">
            <div>
              <label className={labelClass}>Name</label>
              <input
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. Strict Reviewer, Bug Hunter"
                autoFocus
                className={inputClass}
              />
            </div>
            <div className="w-20">
              <label className={labelClass}>Emoji</label>
              <input
                value={emoji}
                onChange={(e) => setEmoji(e.target.value)}
                maxLength={4}
                className={`${inputClass} text-center`}
              />
            </div>
          </div>

          <div>
            <label className={labelClass}>Slug</label>
            <input
              value={slug}
              onChange={(e) => { setSlug(e.target.value); setSlugDirty(true); }}
              placeholder="auto-generated from name"
              className={`${inputClass} font-mono`}
            />
            <p className="text-[11px] text-gray-7 mt-1">Stable identifier. Lowercase letters, digits, and dashes only.</p>
          </div>

          {backendNames.length > 0 && (
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className={labelClass}>CLI</label>
                <select value={cli} onChange={(e) => setCli(e.target.value)} className={`${inputClass} cursor-pointer`}>
                  {backendNames.map((n) => <option key={n} value={n}>{n}</option>)}
                </select>
              </div>
              <div>
                <label className={labelClass}>Model</label>
                <input
                  list={`agents-new-models-${cli}`}
                  value={model}
                  onChange={(e) => setModel(e.target.value)}
                  placeholder="model name (any)"
                  className={inputClass}
                />
                <datalist id={`agents-new-models-${cli}`}>
                  {currentModels.map((m) => <option key={m} value={m} />)}
                </datalist>
              </div>
            </div>
          )}

          <div>
            <label className={labelClass}>System prompt addendum</label>
            <textarea
              value={addendum}
              onChange={(e) => setAddendum(e.target.value)}
              placeholder="Optional. Extra instructions appended to this agent's system prompt for every task it runs."
              rows={4}
              className="w-full px-3 py-2.5 text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-11 placeholder:text-gray-7 focus:outline-none focus:border-gray-6 font-mono"
            />
          </div>

          <div className="flex justify-end gap-2 pt-2 border-t border-gray-3 pb-safe">
            <Button type="button" variant="ghost" size="lg" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" variant="primary" size="lg" disabled={!canSubmit}>
              {saving ? 'Creating...' : 'Create agent'}
            </Button>
          </div>
        </form>
      </div>
    </div>
  );
}
