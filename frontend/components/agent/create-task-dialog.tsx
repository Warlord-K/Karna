'use client';

import { useState, useEffect, useCallback } from 'react';
import { AgentTaskPriority } from '@/lib/agent-tasks';
import { X, Stack, ImageSquare } from '@phosphor-icons/react';
import { ImageDropZone } from './task-attachments';

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

  const handlePaste = useCallback((e: React.ClipboardEvent) => {
    const items = Array.from(e.clipboardData.items);
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
    try {
      await onCreateTask({ title: title.trim(), description: description.trim(), repo: repo || null, priority, cli, model }, images);
      setTitle(''); setDescription(''); setRepo(''); setPriority('medium'); setCli(defaultCli); setModel(defaultModel); setImages([]);
      onClose();
    } catch (error) { console.error(error); } finally { setLoading(false); }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-end sm:items-center justify-center">
      <div className="fixed inset-0 bg-black/50" onClick={onClose} />
      <div className="relative bg-gray-1 rounded-t-2xl sm:rounded-xl shadow-modal w-full sm:max-w-[560px] sm:mx-6 max-h-[90vh] sm:max-h-none overflow-y-auto">
        {/* Drag handle on mobile */}
        <div className="sm:hidden flex justify-center pt-2 pb-0">
          <div className="w-8 h-1 rounded-full bg-gray-6" />
        </div>

        {/* Header */}
        <div className="flex items-center justify-between px-4 sm:px-6 h-13 border-b border-gray-3">
          <span className="text-[15px] font-semibold text-gray-12 tracking-[-0.01em]">New task</span>
          <button onClick={onClose} className="h-8 w-8 sm:h-7 sm:w-7 flex items-center justify-center text-gray-8 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors">
            <X size={16} weight="bold" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-4 sm:p-6 space-y-4 sm:space-y-5">
          <div>
            <label className={labelClass}>Title</label>
            <input
              placeholder="What should the agent build or fix?"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              autoFocus
              className="w-full h-10 sm:h-9 px-3 text-[16px] sm:text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-12 placeholder:text-gray-7 focus:outline-none focus:border-gray-6"
            />
          </div>

          <div>
            <label className={labelClass}>Description</label>
            <textarea
              placeholder="Requirements, context, acceptance criteria..."
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              onPaste={handlePaste}
              rows={3}
              className="w-full px-3 py-2.5 text-[16px] sm:text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-11 placeholder:text-gray-7 focus:outline-none focus:border-gray-6 font-mono"
            />
            <ImageDropZone images={images} onAdd={addImages} onRemove={removeImage} />
            {images.length === 0 && (
              <p className="text-[11px] text-gray-7 mt-1.5 flex items-center gap-1">
                <ImageSquare size={11} weight="bold" /> Paste or drag images here
              </p>
            )}
          </div>

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
              disabled={loading || !title.trim()}
              className="h-10 sm:h-9 px-4 text-[14px] font-medium text-white bg-sun-9 hover:bg-sun-10 text-gray-1 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {loading ? 'Creating...' : 'Create task'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
