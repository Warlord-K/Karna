'use client';

import { useEditor, EditorContent } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Link from '@tiptap/extension-link';
import Placeholder from '@tiptap/extension-placeholder';
import { Markdown } from 'tiptap-markdown';
import { useRef, useEffect } from 'react';

interface MarkdownEditorProps {
  content: string;
  onSave: (markdown: string) => void;
  placeholder?: string;
  className?: string;
}

const extensions = [
  StarterKit.configure({
    heading: { levels: [1, 2, 3, 4] },
    codeBlock: { HTMLAttributes: { class: 'bg-gray-2 border border-gray-4 rounded-md p-4 font-mono text-[13px] text-gray-11 overflow-x-auto' } },
    code: { HTMLAttributes: { class: 'bg-gray-3 text-orange-300 px-1.5 py-0.5 rounded text-[13px] font-mono' } },
    blockquote: { HTMLAttributes: { class: 'border-l-2 border-gray-5 pl-4 text-gray-9 italic' } },
    bulletList: { HTMLAttributes: { class: 'list-disc list-outside ml-5 space-y-1' } },
    orderedList: { HTMLAttributes: { class: 'list-decimal list-outside ml-5 space-y-1' } },
    horizontalRule: { HTMLAttributes: { class: 'border-gray-4 my-6' } },
  }),
  Link.configure({
    openOnClick: false,
    autolink: true,
    HTMLAttributes: { class: 'text-indigo-400 hover:text-indigo-300 underline cursor-pointer' },
  }),
  Placeholder.configure({
    placeholder: ({ node }) => {
      if (node.type.name === 'heading') return 'Heading';
      return '';
    },
  }),
  Markdown.configure({
    transformPastedText: true,
    transformCopiedText: true,
  }),
];

export function MarkdownEditor({ content, onSave, placeholder, className }: MarkdownEditorProps) {
  const lastSavedRef = useRef(content);

  const editor = useEditor({
    extensions: [
      ...extensions.slice(0, 2),
      Placeholder.configure({ placeholder: placeholder || 'Start typing...' }),
      extensions[3],
    ],
    content,
    editorProps: {
      attributes: {
        class: 'focus:outline-none min-h-[2em]',
      },
    },
    onBlur: ({ editor: ed }) => {
      const md = (ed.storage as any).markdown.getMarkdown() as string;
      if (md !== lastSavedRef.current) {
        lastSavedRef.current = md;
        onSave(md);
      }
    },
  });

  // Sync external content changes (e.g. when task switches)
  useEffect(() => {
    if (editor && content !== lastSavedRef.current) {
      lastSavedRef.current = content;
      editor.commands.setContent(content);
    }
  }, [content, editor]);

  if (!editor) return null;

  return (
    <div className={`prose prose-invert max-w-none prose-headings:text-gray-12 prose-p:text-gray-10 prose-p:text-[14px] prose-p:leading-relaxed prose-li:text-gray-10 prose-li:text-[14px] prose-strong:text-gray-11 prose-a:text-indigo-400 prose-code:text-orange-300 prose-code:bg-gray-3 prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded prose-code:text-[13px] prose-pre:bg-gray-2 prose-pre:border prose-pre:border-gray-4 prose-pre:rounded-md prose-blockquote:border-gray-5 prose-blockquote:text-gray-9 prose-hr:border-gray-4 prose-th:text-gray-11 prose-td:text-gray-10 prose-thead:border-gray-4 prose-tr:border-gray-3 ${className || ''}`}>
      <EditorContent editor={editor} />
    </div>
  );
}
