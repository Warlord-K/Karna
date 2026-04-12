'use client';

import { useState } from 'react';
import { X } from '@phosphor-icons/react';

interface AddRepoDialogProps {
  open: boolean;
  onClose: () => void;
  onAdd: (data: { repo: string; branch: string }) => Promise<void>;
}

export function AddRepoDialog({ open, onClose, onAdd }: AddRepoDialogProps) {
  const [repo, setRepo] = useState('');
  const [branch, setBranch] = useState('main');
  const [submitting, setSubmitting] = useState(false);

  if (!open) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!repo.trim()) return;
    setSubmitting(true);
    try {
      await onAdd({ repo: repo.trim(), branch: branch.trim() || 'main' });
      setRepo('');
      setBranch('main');
      onClose();
    } catch {
      // error handled by parent
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="bg-gray-1 border border-gray-3 rounded-xl shadow-elevated w-full max-w-md mx-4">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-gray-3">
          <h2 className="text-[16px] font-semibold text-gray-12">Add Repository</h2>
          <button onClick={onClose} className="text-gray-8 hover:text-gray-12 transition-colors">
            <X size={18} weight="bold" />
          </button>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="px-5 py-4 space-y-4">
          <div>
            <label className="block text-[13px] text-gray-9 mb-1.5">Repository</label>
            <input
              type="text"
              value={repo}
              onChange={(e) => setRepo(e.target.value)}
              placeholder="owner/repo"
              className="w-full h-9 px-3 rounded-lg bg-gray-2 border border-gray-3 text-[14px] text-gray-12 placeholder:text-gray-7 focus:border-sun-9 focus:outline-none font-mono"
              autoFocus
            />
            <p className="text-[12px] text-gray-7 mt-1">GitHub repository in owner/repo format</p>
          </div>

          <div>
            <label className="block text-[13px] text-gray-9 mb-1.5">Branch</label>
            <input
              type="text"
              value={branch}
              onChange={(e) => setBranch(e.target.value)}
              placeholder="main"
              className="w-full h-9 px-3 rounded-lg bg-gray-2 border border-gray-3 text-[14px] text-gray-12 placeholder:text-gray-7 focus:border-sun-9 focus:outline-none font-mono"
            />
          </div>

          <div className="flex justify-end gap-2 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="h-8 px-3.5 text-[13px] text-gray-9 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={!repo.trim() || submitting}
              className="h-8 px-3.5 text-[13px] font-medium bg-sun-9 hover:bg-sun-10 text-gray-1 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {submitting ? 'Adding…' : 'Add Repo'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
