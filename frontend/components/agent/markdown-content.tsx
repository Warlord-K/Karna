'use client';

import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Components } from 'react-markdown';

interface MarkdownContentProps {
  content: string;
  className?: string;
}

export function MarkdownContent({ content, className }: MarkdownContentProps) {
  const components: Components = {
    h1: ({ children }) => (
      <h1 className="text-2xl font-semibold text-gray-12 mt-6 mb-3 tracking-[-0.02em]">{children}</h1>
    ),
    h2: ({ children }) => (
      <h2 className="text-xl font-semibold text-gray-12 mt-6 mb-3 tracking-[-0.02em]">{children}</h2>
    ),
    h3: ({ children }) => (
      <h3 className="text-lg font-semibold text-gray-12 mt-5 mb-2 tracking-[-0.01em]">{children}</h3>
    ),
    h4: ({ children }) => (
      <h4 className="text-base font-semibold text-gray-12 mt-4 mb-2">{children}</h4>
    ),
    h5: ({ children }) => (
      <h5 className="text-sm font-semibold text-gray-12 mt-4 mb-1.5">{children}</h5>
    ),
    h6: ({ children }) => (
      <h6 className="text-sm font-medium text-gray-11 mt-3 mb-1.5">{children}</h6>
    ),
    p: ({ children }) => (
      <p className="text-[14px] text-gray-10 leading-relaxed mb-3">{children}</p>
    ),
    a: ({ href, children }) => (
      <a
        href={href}
        className="text-indigo-400 hover:text-indigo-300 hover:underline transition-colors"
        target="_blank"
        rel="noopener noreferrer"
      >
        {children}
      </a>
    ),
    ul: ({ children }) => (
      <ul className="list-disc list-outside ml-5 mb-3 space-y-1.5 text-[14px] text-gray-10">{children}</ul>
    ),
    ol: ({ children }) => (
      <ol className="list-decimal list-outside ml-5 mb-3 space-y-1.5 text-[14px] text-gray-10">{children}</ol>
    ),
    li: ({ children }) => (
      <li className="leading-relaxed pl-1">{children}</li>
    ),
    blockquote: ({ children }) => (
      <blockquote className="border-l-2 border-gray-5 pl-4 my-3 text-gray-9 italic">
        {children}
      </blockquote>
    ),
    code: ({ inline, className: codeClassName, children, ...props }: any) =>
      inline ? (
        <code className="bg-gray-3 text-orange-300 px-1.5 py-0.5 rounded text-[13px] font-mono" {...props}>
          {children}
        </code>
      ) : (
        <code className={codeClassName} {...props}>
          {children}
        </code>
      ),
    pre: ({ children }) => (
      <pre className="bg-gray-2 border border-gray-4 text-gray-11 p-4 rounded-md overflow-x-auto mb-3 text-[13px] leading-[1.7] font-mono">
        {children}
      </pre>
    ),
    hr: () => <hr className="border-gray-4 my-6" />,
    table: ({ children }) => (
      <div className="overflow-x-auto mb-3 rounded-md border border-gray-4">
        <table className="min-w-full divide-y divide-gray-4 text-[13px]">{children}</table>
      </div>
    ),
    thead: ({ children }) => (
      <thead className="bg-gray-3">{children}</thead>
    ),
    tbody: ({ children }) => (
      <tbody className="divide-y divide-gray-3">{children}</tbody>
    ),
    tr: ({ children }) => <tr className="hover:bg-gray-2 transition-colors">{children}</tr>,
    th: ({ children }) => (
      <th className="px-3 py-2 text-left text-[12px] font-semibold text-gray-11 uppercase tracking-wider">
        {children}
      </th>
    ),
    td: ({ children }) => (
      <td className="px-3 py-2 text-gray-10">{children}</td>
    ),
    strong: ({ children }) => (
      <strong className="font-semibold text-gray-11">{children}</strong>
    ),
    em: ({ children }) => (
      <em className="text-gray-9">{children}</em>
    ),
  };

  return (
    <div className={`prose prose-invert max-w-none ${className || ''}`}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
        {content}
      </ReactMarkdown>
    </div>
  );
}
