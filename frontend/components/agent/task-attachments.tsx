'use client';

import { useState, useCallback, useRef } from 'react';
import { getAttachmentUrl, TaskAttachment } from '@/lib/agent-tasks';
import { useAttachments, useUploadAttachment, useDeleteAttachment } from '@/hooks/use-attachments';
import { X, ImageSquare, Plus, ArrowSquareOut } from '@phosphor-icons/react';
import toast from 'react-hot-toast';

const ALLOWED_TYPES = ['image/jpeg', 'image/png', 'image/gif', 'image/webp'];
const MAX_FILE_SIZE = 5 * 1024 * 1024;
const MAX_IMAGES = 10;

interface TaskAttachmentsProps {
  taskId: string;
  editable?: boolean;
}

export function TaskAttachments({ taskId, editable = true }: TaskAttachmentsProps) {
  const { data: attachments = [], isLoading } = useAttachments(taskId);
  const uploadMutation = useUploadAttachment(taskId);
  const deleteMutation = useDeleteAttachment(taskId);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [dragOver, setDragOver] = useState(false);

  const validateAndUpload = useCallback((files: File[]) => {
    const imageFiles = files.filter(f => ALLOWED_TYPES.includes(f.type));
    if (imageFiles.length === 0) {
      toast.error('Only JPEG, PNG, GIF, and WebP images are supported');
      return;
    }

    const remaining = MAX_IMAGES - attachments.length;
    if (remaining <= 0) {
      toast.error(`Maximum ${MAX_IMAGES} images per task`);
      return;
    }

    const toUpload = imageFiles.slice(0, remaining);
    for (const file of toUpload) {
      if (file.size > MAX_FILE_SIZE) {
        toast.error(`${file.name} is too large (max 5MB)`);
        continue;
      }
      uploadMutation.mutate(file);
    }
  }, [attachments.length, uploadMutation]);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    if (!editable) return;
    const files = Array.from(e.dataTransfer.files);
    validateAndUpload(files);
  }, [editable, validateAndUpload]);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    if (editable) setDragOver(true);
  }, [editable]);

  const handleDragLeave = useCallback(() => {
    setDragOver(false);
  }, []);

  const handleFileSelect = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(e.target.files || []);
    validateAndUpload(files);
    e.target.value = '';
  }, [validateAndUpload]);

  if (isLoading && attachments.length === 0) return null;
  if (!editable && attachments.length === 0) return null;

  return (
    <div
      className={`rounded-lg border transition-colors ${
        dragOver ? 'border-sun-7 bg-sun-3/30' : 'border-gray-3'
      } ${editable ? '' : ''}`}
      onDrop={handleDrop}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
    >
      <div className="flex items-center gap-2 px-3 py-2 border-b border-gray-3">
        <ImageSquare size={14} weight="bold" className="text-gray-8" />
        <span className="text-[12px] font-medium text-gray-8 uppercase tracking-wider">
          Images {attachments.length > 0 && `(${attachments.length})`}
        </span>
        {editable && attachments.length < MAX_IMAGES && (
          <>
            <button
              onClick={() => fileInputRef.current?.click()}
              className="ml-auto h-6 w-6 flex items-center justify-center text-gray-8 hover:text-gray-12 hover:bg-gray-3 rounded transition-colors"
            >
              <Plus size={14} weight="bold" />
            </button>
            <input
              ref={fileInputRef}
              type="file"
              accept="image/jpeg,image/png,image/gif,image/webp"
              multiple
              onChange={handleFileSelect}
              className="hidden"
            />
          </>
        )}
      </div>

      {attachments.length > 0 ? (
        <div className="flex flex-wrap gap-2 p-3">
          {attachments.map((att) => (
            <AttachmentThumbnail
              key={att.id}
              attachment={att}
              taskId={taskId}
              editable={editable}
              onDelete={() => deleteMutation.mutate(att.id)}
            />
          ))}
        </div>
      ) : editable ? (
        <div className="flex flex-col items-center justify-center py-6 text-gray-7">
          <ImageSquare size={24} weight="thin" className="mb-1.5" />
          <p className="text-[12px]">Drag & drop or paste images</p>
        </div>
      ) : null}
    </div>
  );
}

function AttachmentThumbnail({
  attachment,
  taskId,
  editable,
  onDelete,
}: {
  attachment: TaskAttachment;
  taskId: string;
  editable: boolean;
  onDelete: () => void;
}) {
  const url = getAttachmentUrl(taskId, attachment.id);

  return (
    <div className="relative group w-20 h-20 rounded-lg overflow-hidden border border-gray-4 bg-gray-2 flex-shrink-0">
      <a href={url} target="_blank" rel="noopener noreferrer">
        <img
          src={url}
          alt={attachment.filename}
          className="w-full h-full object-cover"
        />
        <div className="absolute inset-0 bg-black/0 group-hover:bg-black/20 transition-colors flex items-center justify-center">
          <ArrowSquareOut size={16} weight="bold" className="text-white opacity-0 group-hover:opacity-100 transition-opacity" />
        </div>
      </a>
      {editable && (
        <button
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            onDelete();
          }}
          className="absolute top-0.5 right-0.5 h-5 w-5 flex items-center justify-center bg-black/60 hover:bg-red-500 text-white rounded-full opacity-0 group-hover:opacity-100 transition-opacity"
        >
          <X size={10} weight="bold" />
        </button>
      )}
    </div>
  );
}

/** Inline image paste/drop zone for use in create-task and other forms */
export function ImageDropZone({
  images,
  onAdd,
  onRemove,
}: {
  images: File[];
  onAdd: (files: File[]) => void;
  onRemove: (index: number) => void;
}) {
  const [dragOver, setDragOver] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    const files = Array.from(e.dataTransfer.files).filter(f => ALLOWED_TYPES.includes(f.type));
    if (files.length > 0) onAdd(files);
  }, [onAdd]);

  if (images.length === 0 && !dragOver) return null;

  return (
    <div
      className={`rounded-lg border transition-colors mt-2 p-2 ${
        dragOver ? 'border-sun-7 bg-sun-3/30' : 'border-gray-4'
      }`}
      onDrop={handleDrop}
      onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
      onDragLeave={() => setDragOver(false)}
    >
      <div className="flex flex-wrap gap-2">
        {images.map((img, i) => (
          <div key={i} className="relative group w-16 h-16 rounded-lg overflow-hidden border border-gray-4 bg-gray-2 flex-shrink-0">
            <img src={URL.createObjectURL(img)} className="w-full h-full object-cover" alt="" />
            <button
              type="button"
              onClick={() => onRemove(i)}
              className="absolute top-0.5 right-0.5 h-5 w-5 flex items-center justify-center bg-black/60 hover:bg-red-500 text-white rounded-full opacity-0 group-hover:opacity-100 transition-opacity"
            >
              <X size={10} weight="bold" />
            </button>
          </div>
        ))}
        <button
          type="button"
          onClick={() => fileInputRef.current?.click()}
          className="w-16 h-16 rounded-lg border border-dashed border-gray-5 flex items-center justify-center text-gray-7 hover:text-gray-11 hover:border-gray-7 transition-colors"
        >
          <Plus size={16} weight="bold" />
        </button>
        <input
          ref={fileInputRef}
          type="file"
          accept="image/jpeg,image/png,image/gif,image/webp"
          multiple
          onChange={(e) => {
            const files = Array.from(e.target.files || []).filter(f => ALLOWED_TYPES.includes(f.type));
            if (files.length > 0) onAdd(files);
            e.target.value = '';
          }}
          className="hidden"
        />
      </div>
    </div>
  );
}
