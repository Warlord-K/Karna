'use client';

import { useState, useEffect, useCallback, useRef } from 'react';
import { AgentTaskPriority } from '@/lib/agent-tasks';
import { X, Stack, ImageSquare, Plus } from '@phosphor-icons/react';
import { MarkdownEditor, MarkdownEditorRef } from './markdown-editor';

export interface BackendConfig {
  models: string[];
  default_model: string;
}

interface CreateTaskDialogProps {
  open: boolean;
  onClose: () => void;
  repos: string[];
  backends: Record<string, BackendConfig>;
  onCreateTask: (data: {
    title: string;
    description: string;
    repo: string | null;
    priority: AgentTaskPriority;
    cli: string | null;
    model: string | null;
  }, images: File[]) => Promise<void>;
}

const ALLOWED_IMAGE_TYPES = ['image/jpeg', 'image/png', 'image/gif', 'image/webp'];
const MAX_FILE_SIZE = 5 * 1024 * 1024;
const MAX_IMAGES = 10;

const PRIORITIES: { value: AgentTaskPriority; label: string; color: string }[] = [
  { value: 'urgent', label: 'Urgent', color: '#e5484d' },
  { value: 'high',   label: 'High',   color: '#e5734e' },
  { value: 'medium', label: 'Medium', color: '#e5a94e' },
  { value: 'low',    label: 'Low',    color: '#7a7a85' },
];

export function CreateTaskDialog({ open, onClose, repos, backends, onCreateTask }: CreateTaskDialogProps) {
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

  useEffect(() => {
    const backend = backends[cli];
    if (backend) setModel(backend.default_model || backend.models[0] || '');
  }, [cli, backends]);

  if (!open) return null;

  const currentModels = backends[cli]?.models || [];
  const selectClass = "w-full h-9 px-3 text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-11 focus:outline-none focus:border-gray-6 cursor-pointer";
  const labelClass = "block text-[12px] font-medium text-gray-8 mb-2 uppercase tracking-wider";

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim()) return;
    setLoading(true);
    const desc = editorRef.current?.getMarkdown() || description;
    try {
      await onCreateTask({ title: title.trim(), description: desc.trim(), repo: repo || null, priority, cli, model }, images);
      setTitle(''); setDescription(''); setRepo(''); setPriority('medium'); setCli(defaultCli); setModel(defaultModel); setImages([]);
      editorRef.current?.clear();
      onClose();
    } catch (error) { console.error(error); } finally { setLoading(false); }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-end sm:items-center justify-center">
      <div className="fixed inset-0 bg-black/50" onClick={onClose} />
      <div className="relative bg-gray-1 rounded-t-2xl sm:rounded-xl shadow-modal w-full sm:max-w-[680px] sm:mx-6 max-h-[92vh] sm:max-h-[88vh] flex flex-col">
        {/* Drag handle on mobile */}
        <div className="sm:hidden flex justify-center pt-2 pb-0">
          <div className="w-8 h-1 rounded-full bg-gray-6" />
        </div>

        {/* Header */}
        <div className="flex items-center justify-between px-4 sm:px-6 h-13 border-b border-gray-3 flex-shrink-0">
          <span className="text-[15px] font-semibold text-gray-12 tracking-[-0.01em]">New task</span>
          <button onClick={onClose} className="h-8 w-8 sm:h-7 sm:w-7 flex items-center justify-center text-gray-8 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors">
            <X size={16} weight="bold" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="flex flex-col flex-1 min-h-0">
          <div className="flex-1 overflow-y-auto p-4 sm:p-6 space-y-4 sm:space-y-5">
            {/* Title */}
            <div>
              <input
                placeholder="What should the agent build or fix?"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                autoFocus
                className="w-full h-10 sm:h-9 px-3 text-[16px] sm:text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-12 placeholder:text-gray-7 focus:outline-none focus:border-gray-6"
              />
            </div>

            {/* Description — rich markdown editor */}
            <div>
              <label className={labelClass}>Description</label>
              <div className="rounded-lg border border-gray-4 bg-gray-2 overflow-hidden focus-within:border-gray-6 transition-colors">
                <MarkdownEditor
                  ref={editorRef}
                  content={description}
                  onSave={setDescription}
                  onPaste={handleEditorPaste}
                  placeholder="Requirements, context, acceptance criteria... (supports markdown)"
                  showToolbar
                  minHeight="min-h-[120px]"
                />
              </div>
            </div>

            {/* Images */}
            <div>
              <div className="flex items-center gap-2 mb-2">
                <ImageSquare size={13} weight="bold" className="text-gray-8" />
                <span className={`text-[12px] font-medium text-gray-8 uppercase tracking-wider`}>
                  Attachments {images.length > 0 && `(${images.length})`}
                </span>
              </div>
              <div
                className="rounded-lg border border-dashed border-gray-4 p-3 hover:border-gray-6 transition-colors"
                onDrop={(e) => {
                  e.preventDefault();
                  const files = Array.from(e.dataTransfer.files).filter(f => ALLOWED_IMAGE_TYPES.includes(f.type));
                  if (files.length > 0) addImages(files);
                }}
                onDragOver={(e) => e.preventDefault()}
              >
                {images.length > 0 ? (
                  <div className="flex flex-wrap gap-2">
                    {images.map((img, i) => (
                      <div key={i} className="relative group w-16 h-16 rounded-lg overflow-hidden border border-gray-4 bg-gray-3 flex-shrink-0">
                        <img src={URL.createObjectURL(img)} className="w-full h-full object-cover" alt="" />
                        <button
                          type="button"
                          onClick={() => removeImage(i)}
                          className="absolute top-0.5 right-0.5 h-5 w-5 flex items-center justify-center bg-black/60 hover:bg-red-500 text-white rounded-full opacity-0 group-hover:opacity-100 transition-opacity"
                        >
                          <X size={10} weight="bold" />
                        </button>
                      </div>
                    ))}
                    {images.length < MAX_IMAGES && (
                      <button
                        type="button"
                        onClick={() => fileInputRef.current?.click()}
                        className="w-16 h-16 rounded-lg border border-dashed border-gray-5 flex items-center justify-center text-gray-7 hover:text-gray-11 hover:border-gray-7 transition-colors"
                      >
                        <Plus size={16} weight="bold" />
                      </button>
                    )}
                  </div>
                ) : (
                  <button
                    type="button"
                    onClick={() => fileInputRef.current?.click()}
                    className="w-full flex flex-col items-center justify-center py-3 text-gray-7 hover:text-gray-9 transition-colors"
                  >
                    <ImageSquare size={20} weight="thin" className="mb-1" />
                    <p className="text-[12px]">Drag, paste, or click to add images</p>
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

            {/* Config row */}
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div>
                <label className={labelClass}>Repository</label>
                <select value={repo} onChange={(e) => setRepo(e.target.value)} className={selectClass}>
                  <option value="">Multi-repo</option>
                  {repos.map((r) => <option key={r} value={r}>{r.split('/').pop()}</option>)}
                </select>
                {!repo && (
                  <p className="text-[11px] text-gray-7 mt-1.5 flex items-center gap-1">
                    <Stack size={11} weight="bold" /> Plans across all repos
                  </p>
                )}
              </div>
              <div>
                <label className={labelClass}>Priority</label>
                <div className="flex gap-1.5">
                  {PRIORITIES.map((p) => (
                    <button
                      key={p.value}
                      type="button"
                      onClick={() => setPriority(p.value)}
                      className={`flex-1 h-10 sm:h-9 rounded-lg text-[12px] font-medium transition-all duration-150 border ${
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
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <div>
                  <label className={labelClass}>Agent</label>
                  <select value={cli} onChange={(e) => setCli(e.target.value)} className={selectClass}>
                    {backendNames.map((name) => <option key={name} value={name}>{name}</option>)}
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
          </div>

          {/* Footer — pinned at bottom */}
          <div className="flex justify-end gap-2.5 px-4 sm:px-6 py-4 border-t border-gray-3 flex-shrink-0 pb-safe">
            <button
              type="button"
              onClick={onClose}
              className="h-10 sm:h-9 px-4 text-[14px] text-gray-9 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={loading || !title.trim()}
              className="h-10 sm:h-9 px-5 text-[14px] font-medium bg-sun-9 hover:bg-sun-10 hover:shadow-[0_0_16px_hsl(40_90%_56%/0.25)] text-gray-1 rounded-lg transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {loading ? 'Creating...' : 'Create task'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
