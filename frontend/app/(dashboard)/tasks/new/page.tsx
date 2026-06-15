'use client';

import { useState, useEffect, useCallback, useRef } from 'react';
import { useRouter } from 'next/navigation';
import { useSession } from 'next-auth/react';
import { useAuthDisabled } from '@/lib/auth-context';
import {
  AgentTaskKind,
  AgentTaskOutputTarget,
  AgentTaskPriority,
  UserSummary,
  userDisplayName,
  createTaskWithImages,
} from '@/lib/agent-tasks';
import { useConfig, useUsers, useAgents, taskKeys } from '@/hooks/use-tasks';
import { useQueryClient } from '@tanstack/react-query';
import { MarkdownEditor, MarkdownEditorRef } from '@/components/agent/markdown-editor';
import {
  ArrowLeft, Stack, ImageSquare, Plus, X, CaretDown, Robot, User,
} from '@phosphor-icons/react';
import toast from 'react-hot-toast';
import { Button } from '@/components/ui/button';

const ALLOWED_IMAGE_TYPES = ['image/jpeg', 'image/png', 'image/gif', 'image/webp'];
const MAX_FILE_SIZE = 5 * 1024 * 1024;
const MAX_IMAGES = 10;

const PRIORITIES: { value: AgentTaskPriority; label: string; color: string }[] = [
  { value: 'urgent', label: 'Urgent', color: '#e5484d' },
  { value: 'high',   label: 'High',   color: '#e5734e' },
  { value: 'medium', label: 'Medium', color: '#e5a94e' },
  { value: 'low',    label: 'Low',    color: '#7a7a85' },
];

const TASK_KINDS: { value: AgentTaskKind; label: string }[] = [
  { value: 'code', label: 'Code' },
  { value: 'doc', label: 'Doc' },
  { value: 'research', label: 'Research' },
  { value: 'ops', label: 'Ops' },
];

const OUTPUT_TARGETS: { value: AgentTaskOutputTarget; label: string }[] = [
  { value: 'none', label: 'None' },
  { value: 'notification', label: 'Notification' },
  { value: 'linear_comment', label: 'Linear comment' },
  { value: 'linear_doc', label: 'Linear doc' },
  { value: 'slack_message', label: 'Slack message' },
];

export default function NewTaskPage() {
  const router = useRouter();
  const queryClient = useQueryClient();

  const authDisabled = useAuthDisabled();
  const { status: authStatus } = useSession();
  const isReady = authDisabled || authStatus === 'authenticated';
  const { data: config } = useConfig(isReady);
  const { data: users = [] } = useUsers(isReady);
  const { data: agents = [] } = useAgents(isReady);

  const repos = config?.repos ?? [];
  const backends = config?.backends ?? {};
  const backendNames = Object.keys(backends);
  const defaultCli = backendNames[0] || 'claude';
  const defaultModel = backends[defaultCli]?.default_model || '';

  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [repo, setRepo] = useState<string>('');
  const [taskKind, setTaskKind] = useState<AgentTaskKind>('code');
  const [outputTarget, setOutputTarget] = useState<AgentTaskOutputTarget>('none');
  const [priority, setPriority] = useState<AgentTaskPriority>('medium');
  const [cli, setCli] = useState(defaultCli);
  const [model, setModel] = useState(defaultModel);
  // Encoded picker value: "" = any agent, "agent:<id>" = specific agent profile,
  // "user:<id>" = human assignee.
  const [assignee, setAssignee] = useState<string>('');
  // Per-stage agent profile overrides ("" = same as assigned/default).
  const [plannerAgent, setPlannerAgent] = useState<string>('');
  const [implementerAgent, setImplementerAgent] = useState<string>('');
  const [reviewerAgent, setReviewerAgent] = useState<string>('');
  const [showStages, setShowStages] = useState(false);
  const [loading, setLoading] = useState(false);
  const [images, setImages] = useState<File[]>([]);
  const editorRef = useRef<MarkdownEditorRef>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const backend = backends[cli];
    if (backend) setModel(backend.default_model || backend.models[0] || '');
  }, [cli, backends]);

  // Init cli/model when config loads
  useEffect(() => {
    if (backendNames.length > 0 && !backendNames.includes(cli)) {
      setCli(backendNames[0]);
    }
  }, [backendNames, cli]);

  useEffect(() => {
    if (taskKind === 'code') return;
    // Non-code tasks should not carry repo or per-stage code-flow overrides.
    setRepo('');
    setPlannerAgent('');
    setImplementerAgent('');
    setReviewerAgent('');
    setShowStages(false);
  }, [taskKind]);

  const addImages = useCallback((files: File[]) => {
    const valid = files.filter(f => {
      if (!ALLOWED_IMAGE_TYPES.includes(f.type)) return false;
      if (f.size > MAX_FILE_SIZE) return false;
      return true;
    });
    setImages(prev => [...prev, ...valid].slice(0, MAX_IMAGES));
  }, []);

  const removeImage = useCallback((index: number) => {
    setImages(prev => prev.filter((_, i) => i !== index));
  }, []);

  const handleEditorPaste = useCallback((e: ClipboardEvent) => {
    const items = Array.from(e.clipboardData?.items || []);
    const imageFiles = items
      .filter(item => item.type.startsWith('image/'))
      .map(item => item.getAsFile())
      .filter(Boolean) as File[];
    if (imageFiles.length > 0) {
      e.preventDefault();
      addImages(imageFiles);
    }
  }, [addImages]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim()) return;
    setLoading(true);
    const desc = editorRef.current?.getMarkdown() || description;
    const [kind, id] = assignee ? assignee.split(':') : ['', ''];
    try {
      await createTaskWithImages(
        {
          title: title.trim(),
          description: desc.trim(),
          repo: taskKind === 'code' ? repo || null : null,
          priority,
          cli,
          model,
          kind: taskKind,
          output_target: taskKind === 'code' ? 'none' : outputTarget,
          assignee_user_id: kind === 'user' ? id : null,
          assigned_agent_id: kind === 'agent' ? id : null,
          planner_agent_id: taskKind === 'code' ? plannerAgent || null : null,
          implementer_agent_id: taskKind === 'code' ? implementerAgent || null : null,
          reviewer_agent_id: taskKind === 'code' ? reviewerAgent || null : null,
        },
        images,
      );
      queryClient.invalidateQueries({ queryKey: taskKeys.lists() });
      toast.success('Task created');
      router.push('/');
    } catch (error) {
      console.error(error);
      toast.error('Failed to create task');
    } finally {
      setLoading(false);
    }
  };

  const currentModels = backends[cli]?.models || [];
  const selectClass = "w-full h-9 px-3 text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-11 transition-smooth focus-ring focus-visible:border-gray-6 cursor-pointer appearance-none";
  const labelClass = "block text-[12px] font-medium text-gray-8 mb-2 uppercase tracking-wider";

  return (
    <div className="h-full overflow-y-auto">
      <div className="w-full px-4 sm:px-6 lg:px-8 py-6">
        {/* Header */}
        <div className="flex items-center justify-between mb-8">
          <div className="flex items-center gap-3">
            <button
              onClick={() => router.push('/')}
              className="h-8 w-8 flex items-center justify-center text-gray-8 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors"
            >
              <ArrowLeft size={18} weight="bold" />
            </button>
            <h1 className="text-[20px] font-semibold text-gray-12 tracking-[-0.02em]">New task</h1>
          </div>
        </div>

        <form onSubmit={handleSubmit} className="max-w-4xl space-y-6">
          {/* Title */}
          <div>
            <input
              placeholder="What should the agent build or fix?"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              autoFocus
              className="w-full h-12 px-4 text-[18px] sm:text-[16px] rounded-xl bg-gray-2 border border-gray-4 text-gray-12 placeholder:text-gray-7 transition-smooth focus-ring focus-visible:border-gray-6"
            />
          </div>

          {/* Description — rich markdown editor */}
          <div>
            <label className={labelClass}>Description</label>
            <div className="rounded-xl border border-gray-4 bg-gray-2 overflow-hidden focus-within:border-gray-6 transition-colors">
              <MarkdownEditor
                ref={editorRef}
                content={description}
                onSave={setDescription}
                onPaste={handleEditorPaste}
                placeholder="Requirements, context, acceptance criteria... (supports full markdown)"
                showToolbar
                minHeight="min-h-[240px]"
              />
            </div>
            <p className="text-[11px] text-gray-7 mt-2">
              Supports markdown: headings, bold, italic, lists, code blocks, links. Paste images directly into the editor.
            </p>
          </div>

          {/* Attachments */}
          <div>
            <div className="flex items-center gap-2 mb-2">
              <ImageSquare size={13} weight="bold" className="text-gray-8" />
              <span className={labelClass + ' mb-0'}>
                Attachments {images.length > 0 && `(${images.length}/${MAX_IMAGES})`}
              </span>
            </div>
            <div
              className={`rounded-xl border border-dashed transition-colors ${
                images.length > 0 ? 'border-gray-4 p-3' : 'border-gray-4 hover:border-gray-6'
              }`}
              onDrop={(e) => {
                e.preventDefault();
                const files = Array.from(e.dataTransfer.files).filter(f => ALLOWED_IMAGE_TYPES.includes(f.type));
                if (files.length > 0) addImages(files);
              }}
              onDragOver={(e) => e.preventDefault()}
            >
              {images.length > 0 ? (
                <div className="flex flex-wrap gap-2.5">
                  {images.map((img, i) => (
                    <div key={i} className="relative group w-20 h-20 rounded-lg overflow-hidden border border-gray-4 bg-gray-3 flex-shrink-0">
                      <img src={URL.createObjectURL(img)} className="w-full h-full object-cover" alt="" />
                      <button
                        type="button"
                        onClick={() => removeImage(i)}
                        className="absolute top-1 right-1 h-5 w-5 flex items-center justify-center bg-black/60 hover:bg-red-500 text-white rounded-full opacity-0 group-hover:opacity-100 transition-opacity"
                      >
                        <X size={10} weight="bold" />
                      </button>
                    </div>
                  ))}
                  {images.length < MAX_IMAGES && (
                    <button
                      type="button"
                      onClick={() => fileInputRef.current?.click()}
                      className="w-20 h-20 rounded-lg border border-dashed border-gray-5 flex items-center justify-center text-gray-7 hover:text-gray-11 hover:border-gray-7 transition-colors"
                    >
                      <Plus size={18} weight="bold" />
                    </button>
                  )}
                </div>
              ) : (
                <button
                  type="button"
                  onClick={() => fileInputRef.current?.click()}
                  className="w-full flex flex-col items-center justify-center py-6 text-gray-7 hover:text-gray-9 transition-colors"
                >
                  <ImageSquare size={28} weight="thin" className="mb-2" />
                  <p className="text-[13px]">Drag, paste, or click to add images</p>
                  <p className="text-[11px] text-gray-6 mt-1">JPEG, PNG, GIF, WebP up to 5MB</p>
                </button>
              )}
              <input
                ref={fileInputRef}
                type="file"
                accept="image/jpeg,image/png,image/gif,image/webp"
                multiple
                onChange={(e) => {
                  const files = Array.from(e.target.files || []).filter(f => ALLOWED_IMAGE_TYPES.includes(f.type));
                  if (files.length > 0) addImages(files);
                  e.target.value = '';
                }}
                className="hidden"
              />
            </div>
          </div>

          {/* Config section */}
          <div className="border-t border-gray-3 pt-6">
            {/* Assignee — any agent (default), a specific agent profile, or a human */}
            <div className="mb-5">
              <label className={labelClass}>Assigned to</label>
              <div className="relative">
                <select
                  value={assignee}
                  onChange={(e) => setAssignee(e.target.value)}
                  className={`${selectClass} pl-9`}
                >
                  <option value="">Any agent</option>
                  {agents.length > 0 && (
                    <optgroup label="Agents">
                      {agents.map((a) => (
                        <option key={a.id} value={`agent:${a.id}`} disabled={!!a.paused_reason}>
                          {a.avatar_emoji} {a.name}{a.paused_reason ? ' (paused)' : ''}
                        </option>
                      ))}
                    </optgroup>
                  )}
                  {users.length > 0 && (
                    <optgroup label="Humans">
                      {users.map((u: UserSummary) => (
                        <option key={u.id} value={`user:${u.id}`}>{userDisplayName(u)}</option>
                      ))}
                    </optgroup>
                  )}
                </select>
                <div className="absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none text-gray-8">
                  {assignee.startsWith('user:') ? <User size={14} weight="bold" /> : <Robot size={14} weight="bold" />}
                </div>
                <CaretDown size={14} weight="bold" className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-7 pointer-events-none" />
              </div>
              {assignee.startsWith('user:') && (
                <p className="text-[11px] text-gray-7 mt-1.5 flex items-center gap-1">
                  <User size={11} weight="bold" /> Agent will skip this task until reassigned
                </p>
              )}
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-5">
              <div>
                <label className={labelClass}>Kind</label>
                <div className="relative">
                  <select value={taskKind} onChange={(e) => setTaskKind(e.target.value as AgentTaskKind)} className={selectClass}>
                    {TASK_KINDS.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}
                  </select>
                  <CaretDown size={14} weight="bold" className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-7 pointer-events-none" />
                </div>
              </div>

              {taskKind !== 'code' && (
                <div>
                  <label className={labelClass}>Output target</label>
                  <div className="relative">
                    <select
                      value={outputTarget}
                      onChange={(e) => setOutputTarget(e.target.value as AgentTaskOutputTarget)}
                      className={selectClass}
                    >
                      {OUTPUT_TARGETS.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}
                    </select>
                    <CaretDown size={14} weight="bold" className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-7 pointer-events-none" />
                  </div>
                </div>
              )}
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-5 mt-5">
              {/* Repository */}
              <div>
                <label className={labelClass}>Repository</label>
                <div className="relative">
                  <select
                    value={repo}
                    onChange={(e) => setRepo(e.target.value)}
                    className={selectClass}
                    disabled={taskKind !== 'code'}
                  >
                    <option value="">Multi-repo (auto-detect)</option>
                    {repos.map((r) => <option key={r} value={r}>{r}</option>)}
                  </select>
                  <CaretDown size={14} weight="bold" className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-7 pointer-events-none" />
                </div>
                {taskKind === 'code' && !repo && (
                  <p className="text-[11px] text-gray-7 mt-1.5 flex items-center gap-1">
                    <Stack size={11} weight="bold" /> Agent decides which repos need changes
                  </p>
                )}
                {taskKind !== 'code' && (
                  <p className="text-[11px] text-gray-7 mt-1.5">Non-code tasks run without git worktrees or PRs.</p>
                )}
              </div>

              {/* Priority */}
              <div>
                <label className={labelClass}>Priority</label>
                <div className="flex gap-1.5">
                  {PRIORITIES.map((p) => (
                    <button
                      key={p.value}
                      type="button"
                      onClick={() => setPriority(p.value)}
                      className={`flex-1 h-9 rounded-lg text-[12px] font-medium transition-all duration-150 border ${
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
            </div>

            {backendNames.length > 0 && (
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-5 mt-5">
                <div>
                  <label className={labelClass}>Agent</label>
                  <div className="relative">
                    <select value={cli} onChange={(e) => setCli(e.target.value)} className={selectClass}>
                      {backendNames.map((name) => <option key={name} value={name}>{name}</option>)}
                    </select>
                    <CaretDown size={14} weight="bold" className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-7 pointer-events-none" />
                  </div>
                </div>
                <div>
                  <label className={labelClass}>Model</label>
                  <input
                    list={`tasks-new-models-${cli}`}
                    value={model}
                    onChange={(e) => setModel(e.target.value)}
                    placeholder="model name (any)"
                    className="w-full h-9 px-3 text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-11 placeholder:text-gray-7 focus:outline-none focus:border-gray-6"
                  />
                  <datalist id={`tasks-new-models-${cli}`}>
                    {currentModels.map((m) => <option key={m} value={m} />)}
                  </datalist>
                </div>
              </div>
            )}

            {/* Per-stage agents (advanced) — run scope / implement / review on
                different agent profiles. Empty = use the assigned agent / default. */}
            {taskKind === 'code' && agents.length > 0 && (
              <div className="mt-5">
                <button
                  type="button"
                  onClick={() => setShowStages((v) => !v)}
                  className="flex items-center gap-1.5 text-[12px] font-medium text-gray-8 hover:text-gray-11 uppercase tracking-wider transition-colors"
                >
                  <CaretDown
                    size={13}
                    weight="bold"
                    className={`transition-transform ${showStages ? 'rotate-0' : '-rotate-90'}`}
                  />
                  Per-stage agents
                </button>
                {showStages && (
                  <>
                    <p className="text-[11px] text-gray-7 mt-2 mb-3">
                      Run each stage on a different agent profile (e.g. plan with one, implement with another, self-review with a third). Leave on &ldquo;Same as assigned&rdquo; to use the task&rsquo;s agent.
                    </p>
                    <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
                      {([
                        ['Planner', plannerAgent, setPlannerAgent],
                        ['Implementer', implementerAgent, setImplementerAgent],
                        ['Reviewer', reviewerAgent, setReviewerAgent],
                      ] as const).map(([label, value, setter]) => (
                        <div key={label}>
                          <label className={labelClass}>{label}</label>
                          <div className="relative">
                            <select value={value} onChange={(e) => setter(e.target.value)} className={selectClass}>
                              <option value="">Same as assigned</option>
                              {agents.map((a) => (
                                <option key={a.id} value={a.id} disabled={!!a.paused_reason}>
                                  {a.avatar_emoji} {a.name}{a.paused_reason ? ' (paused)' : ''}
                                </option>
                              ))}
                            </select>
                            <CaretDown size={14} weight="bold" className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-7 pointer-events-none" />
                          </div>
                        </div>
                      ))}
                    </div>
                  </>
                )}
              </div>
            )}
          </div>

          {/* Submit footer */}
          <div className="flex items-center justify-between pt-6 border-t border-gray-3 pb-safe">
            <Button type="button" variant="ghost" size="lg" onClick={() => router.push('/')}>
              Cancel
            </Button>
            <Button type="submit" variant="primary" size="lg" disabled={loading || !title.trim()} className="px-6 h-10">
              {loading ? 'Creating...' : 'Create task'}
            </Button>
          </div>
        </form>
      </div>
    </div>
  );
}
