'use client';

import { useState, useEffect, useCallback, useRef } from 'react';
import { useRouter } from 'next/navigation';
import { useSession } from 'next-auth/react';
import { useAuthDisabled } from '@/lib/auth-context';
import { AgentTaskPriority, createTaskWithImages } from '@/lib/agent-tasks';
import { useConfig, taskKeys } from '@/hooks/use-tasks';
import { useQueryClient } from '@tanstack/react-query';
import { MarkdownEditor, MarkdownEditorRef } from '@/components/agent/markdown-editor';
import {
  ArrowLeft, Stack, ImageSquare, Plus, X, CaretDown,
} from '@phosphor-icons/react';
import toast from 'react-hot-toast';

const ALLOWED_IMAGE_TYPES = ['image/jpeg', 'image/png', 'image/gif', 'image/webp'];
const MAX_FILE_SIZE = 5 * 1024 * 1024;
const MAX_IMAGES = 10;

const PRIORITIES: { value: AgentTaskPriority; label: string; color: string }[] = [
  { value: 'urgent', label: 'Urgent', color: '#e5484d' },
  { value: 'high',   label: 'High',   color: '#e5734e' },
  { value: 'medium', label: 'Medium', color: '#e5a94e' },
  { value: 'low',    label: 'Low',    color: '#7a7a85' },
];

export default function NewTaskPage() {
  const router = useRouter();
  const queryClient = useQueryClient();

  const authDisabled = useAuthDisabled();
  const { status: authStatus } = useSession();
  const isReady = authDisabled || authStatus === 'authenticated';
  const { data: config } = useConfig(isReady);

  const repos = config?.repos ?? [];
  const backends = config?.backends ?? {};
  const backendNames = Object.keys(backends);
  const defaultCli = backendNames[0] || 'claude';
  const defaultModel = backends[defaultCli]?.default_model || '';

  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [repo, setRepo] = useState<string>('');
  const [priority, setPriority] = useState<AgentTaskPriority>('medium');
  const [cli, setCli] = useState(defaultCli);
  const [model, setModel] = useState(defaultModel);
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
    try {
      await createTaskWithImages(
        { title: title.trim(), description: desc.trim(), repo: repo || null, priority, cli, model },
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
  const selectClass = "w-full h-9 px-3 text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-11 focus:outline-none focus:border-gray-6 cursor-pointer appearance-none";
  const labelClass = "block text-[12px] font-medium text-gray-8 mb-2 uppercase tracking-wider";

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-3xl mx-auto px-4 sm:px-6 py-6">
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

        <form onSubmit={handleSubmit} className="space-y-6">
          {/* Title */}
          <div>
            <input
              placeholder="What should the agent build or fix?"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              autoFocus
              className="w-full h-12 px-4 text-[18px] sm:text-[16px] rounded-xl bg-gray-2 border border-gray-4 text-gray-12 placeholder:text-gray-7 focus:outline-none focus:border-gray-6 transition-colors"
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
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-5">
              {/* Repository */}
              <div>
                <label className={labelClass}>Repository</label>
                <div className="relative">
                  <select value={repo} onChange={(e) => setRepo(e.target.value)} className={selectClass}>
                    <option value="">Multi-repo (auto-detect)</option>
                    {repos.map((r) => <option key={r} value={r}>{r}</option>)}
                  </select>
                  <CaretDown size={14} weight="bold" className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-7 pointer-events-none" />
                </div>
                {!repo && (
                  <p className="text-[11px] text-gray-7 mt-1.5 flex items-center gap-1">
                    <Stack size={11} weight="bold" /> Agent decides which repos need changes
                  </p>
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
                  <div className="relative">
                    <select value={model} onChange={(e) => setModel(e.target.value)} className={selectClass}>
                      {currentModels.map((m) => <option key={m} value={m}>{m}</option>)}
                    </select>
                    <CaretDown size={14} weight="bold" className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-7 pointer-events-none" />
                  </div>
                </div>
              </div>
            )}
          </div>

          {/* Submit footer */}
          <div className="flex items-center justify-between pt-6 border-t border-gray-3 pb-safe">
            <button
              type="button"
              onClick={() => router.push('/')}
              className="h-9 px-4 text-[14px] text-gray-9 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={loading || !title.trim()}
              className="h-10 px-6 text-[14px] font-medium bg-sun-9 hover:bg-sun-10 hover:shadow-[0_0_16px_hsl(40_90%_56%/0.25)] text-gray-1 rounded-lg transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {loading ? 'Creating...' : 'Create task'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
