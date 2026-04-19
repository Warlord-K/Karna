'use client';

import { useEditor, EditorContent, Editor } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Link from '@tiptap/extension-link';
import Placeholder from '@tiptap/extension-placeholder';
import { Markdown } from 'tiptap-markdown';
import { useRef, useEffect, useImperativeHandle, forwardRef, useCallback } from 'react';
import {
  TextBolder, TextItalic, TextStrikethrough, Code, CodeBlock,
  ListBullets, ListNumbers, Quotes, LineSegment, TextHOne, TextHTwo, TextHThree,
} from '@phosphor-icons/react';

interface MarkdownEditorProps {
  content: string;
  onSave: (markdown: string) => void;
  placeholder?: string;
  className?: string;
  showToolbar?: boolean;
  minHeight?: string;
  onPaste?: (e: ClipboardEvent) => void;
}

export interface MarkdownEditorRef {
  getMarkdown: () => string;
  clear: () => void;
}

const baseExtensions = [
  StarterKit.configure({
    heading: { levels: [1, 2, 3] },
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
  Markdown.configure({
    transformPastedText: true,
    transformCopiedText: true,
  }),
];

const proseClasses = 'prose prose-invert max-w-none prose-headings:text-gray-12 prose-p:text-gray-10 prose-p:text-[14px] prose-p:leading-relaxed prose-li:text-gray-10 prose-li:text-[14px] prose-strong:text-gray-11 prose-a:text-indigo-400 prose-code:text-orange-300 prose-code:bg-gray-3 prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded prose-code:text-[13px] prose-pre:bg-gray-2 prose-pre:border prose-pre:border-gray-4 prose-pre:rounded-md prose-blockquote:border-gray-5 prose-blockquote:text-gray-9 prose-hr:border-gray-4 prose-th:text-gray-11 prose-td:text-gray-10 prose-thead:border-gray-4 prose-tr:border-gray-3';

function ToolbarButton({ active, onClick, children, title }: { active?: boolean; onClick: () => void; children: React.ReactNode; title: string }) {
  return (
    <button
      type="button"
      title={title}
      onMouseDown={(e) => { e.preventDefault(); onClick(); }}
      className={`h-7 w-7 flex items-center justify-center rounded-md transition-colors ${
        active ? 'bg-gray-4 text-gray-12' : 'text-gray-7 hover:text-gray-11 hover:bg-gray-3'
      }`}
    >
      {children}
    </button>
  );
}

function EditorToolbar({ editor }: { editor: Editor }) {
  return (
    <div className="flex items-center gap-0.5 px-2 py-1.5 border-b border-gray-4 flex-wrap">
      <ToolbarButton title="Heading 1" active={editor.isActive('heading', { level: 1 })} onClick={() => editor.chain().focus().toggleHeading({ level: 1 }).run()}>
        <TextHOne size={15} weight="bold" />
      </ToolbarButton>
      <ToolbarButton title="Heading 2" active={editor.isActive('heading', { level: 2 })} onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}>
        <TextHTwo size={15} weight="bold" />
      </ToolbarButton>
      <ToolbarButton title="Heading 3" active={editor.isActive('heading', { level: 3 })} onClick={() => editor.chain().focus().toggleHeading({ level: 3 }).run()}>
        <TextHThree size={15} weight="bold" />
      </ToolbarButton>

      <div className="w-px h-4 bg-gray-5 mx-1" />

      <ToolbarButton title="Bold" active={editor.isActive('bold')} onClick={() => editor.chain().focus().toggleBold().run()}>
        <TextBolder size={15} weight="bold" />
      </ToolbarButton>
      <ToolbarButton title="Italic" active={editor.isActive('italic')} onClick={() => editor.chain().focus().toggleItalic().run()}>
        <TextItalic size={15} weight="bold" />
      </ToolbarButton>
      <ToolbarButton title="Strikethrough" active={editor.isActive('strike')} onClick={() => editor.chain().focus().toggleStrike().run()}>
        <TextStrikethrough size={15} weight="bold" />
      </ToolbarButton>
      <ToolbarButton title="Inline code" active={editor.isActive('code')} onClick={() => editor.chain().focus().toggleCode().run()}>
        <Code size={15} weight="bold" />
      </ToolbarButton>

      <div className="w-px h-4 bg-gray-5 mx-1" />

      <ToolbarButton title="Bullet list" active={editor.isActive('bulletList')} onClick={() => editor.chain().focus().toggleBulletList().run()}>
        <ListBullets size={15} weight="bold" />
      </ToolbarButton>
      <ToolbarButton title="Numbered list" active={editor.isActive('orderedList')} onClick={() => editor.chain().focus().toggleOrderedList().run()}>
        <ListNumbers size={15} weight="bold" />
      </ToolbarButton>
      <ToolbarButton title="Blockquote" active={editor.isActive('blockquote')} onClick={() => editor.chain().focus().toggleBlockquote().run()}>
        <Quotes size={15} weight="bold" />
      </ToolbarButton>
      <ToolbarButton title="Code block" active={editor.isActive('codeBlock')} onClick={() => editor.chain().focus().toggleCodeBlock().run()}>
        <CodeBlock size={15} weight="bold" />
      </ToolbarButton>
      <ToolbarButton title="Horizontal rule" onClick={() => editor.chain().focus().setHorizontalRule().run()}>
        <LineSegment size={15} weight="bold" />
      </ToolbarButton>
    </div>
  );
}

export const MarkdownEditor = forwardRef<MarkdownEditorRef, MarkdownEditorProps>(function MarkdownEditor(
  { content, onSave, placeholder, className, showToolbar, minHeight, onPaste },
  ref
) {
  const lastSavedRef = useRef(content);

  const editor = useEditor({
    extensions: [
      ...baseExtensions,
      Placeholder.configure({ placeholder: placeholder || 'Start typing...' }),
    ],
    content,
    editorProps: {
      attributes: {
        class: `focus:outline-none ${minHeight || 'min-h-[2em]'}`,
      },
      handlePaste: onPaste ? (_view, event) => {
        onPaste(event as unknown as ClipboardEvent);
        return false; // let tiptap handle text paste normally
      } : undefined,
    },
    onBlur: ({ editor: ed }) => {
      const md = (ed.storage as any).markdown.getMarkdown() as string;
      if (md !== lastSavedRef.current) {
        lastSavedRef.current = md;
        onSave(md);
      }
    },
  });

  useImperativeHandle(ref, () => ({
    getMarkdown: () => {
      if (!editor) return '';
      return (editor.storage as any).markdown.getMarkdown() as string;
    },
    clear: () => {
      if (editor) {
        editor.commands.clearContent();
        lastSavedRef.current = '';
      }
    },
  }), [editor]);

  // Sync external content changes (e.g. when task switches)
  useEffect(() => {
    if (editor && content !== lastSavedRef.current) {
      lastSavedRef.current = content;
      editor.commands.setContent(content);
    }
  }, [content, editor]);

  if (!editor) return null;

  return (
    <div className={`${proseClasses} ${className || ''}`}>
      {showToolbar && <EditorToolbar editor={editor} />}
      <div className={showToolbar ? 'px-3 py-2.5' : ''}>
        <EditorContent editor={editor} />
      </div>
    </div>
  );
});
