'use client';

import { useEffect, useRef } from 'react';
import { useQuery } from '@tanstack/react-query';
import { PrReview, PrReviewLog, fetchReviewLogs } from '@/lib/repos';
import { X, ArrowSquareOut, Lightning, WarningCircle, Terminal, Article } from '@phosphor-icons/react';
import { format } from 'date-fns';

interface ReviewLogModalProps {
  repoId: string;
  review: PrReview;
  onClose: () => void;
}

export function ReviewLogModal({ repoId, review, onClose }: ReviewLogModalProps) {
  const isLive = review.status === 'running' || review.status === 'pending';

  const { data: logs = [], isLoading } = useQuery<PrReviewLog[]>({
    queryKey: ['repos', repoId, 'reviews', review.id, 'logs'],
    queryFn: ({ signal }) => fetchReviewLogs(repoId, review.id, signal),
    refetchInterval: isLive ? 2000 : false,
  });

  const endRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [logs]);

  const statusBadge: Record<string, string> = {
    running: 'bg-amber-500/15 text-amber-400 border-amber-500/30 animate-pulse',
    pending: 'bg-amber-500/15 text-amber-400 border-amber-500/30',
    completed: 'bg-green-500/15 text-green-400 border-green-500/30',
    failed: 'bg-red-500/15 text-red-400 border-red-500/30',
    skipped: 'bg-gray-3 text-gray-8 border-gray-5',
  };

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={onClose}>
      <div
        className="bg-gray-1 border border-gray-3 rounded-xl shadow-elevated w-full max-w-3xl mx-4 max-h-[85vh] flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-5 py-4 border-b border-gray-3 flex-shrink-0">
          <div className="flex items-center gap-2 min-w-0">
            <h2 className="text-[15px] font-semibold text-gray-12 truncate">
              PR review · <span className="font-mono">{review.repo}#{review.pr_number}</span>
            </h2>
            <span className={`inline-flex items-center px-1.5 h-5 rounded text-[10px] border ${statusBadge[review.status] || statusBadge.pending}`}>
              {review.status}
            </span>
          </div>
          <div className="flex items-center gap-1 flex-shrink-0">
            {review.pr_url && (
              <a
                href={review.pr_url}
                target="_blank"
                rel="noopener noreferrer"
                className="h-8 px-2 text-[12px] text-gray-9 hover:text-gray-12 hover:bg-gray-3 rounded-md transition-colors flex items-center gap-1"
              >
                Open PR <ArrowSquareOut size={12} weight="bold" />
              </a>
            )}
            <button onClick={onClose} className="h-8 w-8 flex items-center justify-center text-gray-8 hover:text-gray-12 transition-colors">
              <X size={18} weight="bold" />
            </button>
          </div>
        </div>

        <div className="px-5 py-3 border-b border-gray-3 grid grid-cols-2 sm:grid-cols-4 gap-3 text-[11px] flex-shrink-0">
          <Meta label="Author" value={review.author ? `@${review.author}` : '—'} />
          <Meta label="Commit" value={review.head_sha.slice(0, 8)} mono />
          <Meta label="Cost" value={`$${review.cost_usd.toFixed(4)}`} mono />
          <Meta
            label="Duration"
            value={
              review.completed_at && review.created_at
                ? `${Math.max(0, Math.round((new Date(review.completed_at).getTime() - new Date(review.created_at).getTime()) / 1000))}s`
                : isLive
                  ? 'running…'
                  : '—'
            }
          />
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-3 space-y-1">
          {isLoading && logs.length === 0 ? (
            <div className="text-[12px] text-gray-7">Loading logs…</div>
          ) : logs.length === 0 ? (
            <div className="text-[12px] text-gray-7">
              No log entries yet{isLive ? '. Reviewer is starting…' : '.'}
            </div>
          ) : (
            logs.map((log) => <LogLine key={log.id} log={log} />)
          )}
          {review.error_message && (
            <div className="mt-3 bg-red-400/10 border border-red-400/20 rounded-lg px-3 py-2 text-[12px] text-red-300 font-mono whitespace-pre-wrap">
              {review.error_message}
            </div>
          )}
          <div ref={endRef} />
        </div>
      </div>
    </div>
  );
}

function Meta({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div>
      <div className="text-gray-7 uppercase tracking-wider">{label}</div>
      <div className={`text-gray-11 mt-0.5 ${mono ? 'font-mono' : ''}`}>{value}</div>
    </div>
  );
}

function LogLine({ log }: { log: PrReviewLog }) {
  const iconByType: Record<string, React.ReactNode> = {
    tool: <Terminal size={11} weight="bold" className="text-sun-9" />,
    command: <Lightning size={11} weight="bold" className="text-sun-9" />,
    output: <Article size={11} weight="bold" className="text-gray-7" />,
    error: <WarningCircle size={11} weight="fill" className="text-red-400" />,
    warning: <WarningCircle size={11} weight="bold" className="text-amber-400" />,
    info: <Article size={11} weight="bold" className="text-gray-7" />,
  };
  const icon = (log.log_type && iconByType[log.log_type]) || iconByType.info;
  const messageColor = log.log_type === 'error'
    ? 'text-red-300'
    : log.log_type === 'warning'
      ? 'text-amber-300'
      : 'text-gray-11';
  return (
    <div className="flex items-start gap-2 text-[12px] font-mono">
      <span className="text-gray-7 flex-shrink-0 mt-[3px]">
        {log.created_at ? format(new Date(log.created_at), 'HH:mm:ss') : ''}
      </span>
      <span className="flex-shrink-0 mt-[3px]">{icon}</span>
      <span className={`${messageColor} whitespace-pre-wrap break-words flex-1 min-w-0`}>{log.message}</span>
    </div>
  );
}
