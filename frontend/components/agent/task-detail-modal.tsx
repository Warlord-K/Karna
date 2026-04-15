'use client';

import { useState, useEffect, useRef } from 'react';
import {
  AgentTask,
  AgentLog,
  AgentTaskPriority,
  AgentTaskStatus,
  PRIORITY_COLORS,
  hasSubtaskDefinitions,
} from '@/lib/agent-tasks';
import {
  useSubtasks,
  useLogs,
  useApproveWithSubtasks,
  usePostComment,
} from '@/hooks/use-tasks';
import {
  Trash, GitPullRequest, ArrowSquareOut, Check, X, Prohibit,
  ChatText, Article, FileText, Lightning, WarningCircle, ArrowCounterClockwise,
  Clock, Stack, Terminal,
} from '@phosphor-icons/react';
import toast from 'react-hot-toast';
import { MarkdownEditor } from './markdown-editor';
import { TaskAttachments } from './task-attachments';
import { formatDistanceToNow, format } from 'date-fns';

type Tab = 'details' | 'plan' | 'subtasks' | 'activity';

interface TaskDetailModalProps {
  task: AgentTask | null;
  onClose: () => void;
  onUpdate: (id: string, updates: Partial<AgentTask>) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
}

export function TaskDetailModal({ task, onClose, onUpdate, onDelete }: TaskDetailModalProps) {
  const [activeTab, setActiveTab] = useState<Tab>('details');
  const [comment, setComment] = useState('');
  const commentRef = useRef<HTMLTextAreaElement>(null);
  const [loading, setLoading] = useState(false);
  const logsEndRef = useRef<HTMLDivElement>(null);

  const prevTaskId = useRef<string | null>(null);

  useEffect(() => {
    if (task && task.id !== prevTaskId.current) {
      prevTaskId.current = task.id;
      setActiveTab(task.plan_content ? 'plan' : 'details');
      setComment('');
    }
    if (!task) {
      prevTaskId.current = null;
    }
  }, [task]);

  const taskId = task?.id ?? null;

  const { data: subtasks = [] } = useSubtasks(taskId, activeTab === 'subtasks');
  const { data: logs = [], isLoading: logsLoading } = useLogs(taskId, activeTab === 'activity');
  const approveSubtasksMutation = useApproveWithSubtasks();
  const postCommentMutation = usePostComment();

  useEffect(() => {
    logsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [logs]);

  if (!task) return null;

  const handlePriorityChange = async (p: AgentTaskPriority) => { await onUpdate(task.id, { priority: p }); };

  const handleApprovePlan = async () => {
    setLoading(true);
    try {
      if (hasSubtaskDefinitions(task)) {
        const created = await approveSubtasksMutation.mutateAsync(task.id);
        toast.success(`Plan approved \u2014 ${created.length} subtasks created`);
        setActiveTab('subtasks');
      } else {
        await onUpdate(task.id, { status: 'in_progress' });
        toast.success('Plan approved');
      }
    } catch (e: any) { toast.error(e.message || 'Failed'); } finally { setLoading(false); }
  };

  const handleRejectPlan = async () => {
    setActiveTab('activity');
    setTimeout(() => commentRef.current?.focus(), 100);
    toast('Add a comment with your feedback', { icon: '💬' });
  };

  const handlePostComment = async () => {
    if (!comment.trim()) return;
    setLoading(true);
    try {
      await postCommentMutation.mutateAsync({ taskId: task.id, message: comment.trim() });
      setComment('');
      const statusHint = task.status === 'review' ? ' — agent will apply changes'
        : task.status === 'plan_review' ? ' — sent back for re-planning' : '';
      toast.success(`Comment added${statusHint}`);
    } catch (e: any) { toast.error(e.message || 'Failed to post comment'); } finally { setLoading(false); }
  };

  const handleRetry = async () => {
    setLoading(true);
    try { await onUpdate(task.id, { status: 'todo', error_message: null, feedback: null }); toast.success('Retrying'); } finally { setLoading(false); }
  };

  const handleCancel = async () => {
    setLoading(true);
    try { await onUpdate(task.id, { status: 'cancelled' }); toast.success('Task cancelled'); } finally { setLoading(false); }
  };

  const handleDelete = async () => {
    if (!confirm('Delete this task?')) return;
    setLoading(true);
    try { await onDelete(task.id); onClose(); } finally { setLoading(false); }
  };

  const repoName = task.repo ? (task.repo.split('/').pop() || task.repo) : null;

  const tabs: { id: Tab; label: string; icon: React.ReactNode; hidden?: boolean }[] = [
    { id: 'details',  label: 'Details',  icon: <FileText size={16} weight="bold" /> },
    { id: 'plan',     label: 'Plan',     icon: <Article size={16} weight="bold" /> },
    { id: 'subtasks', label: `Subtasks${subtasks.length ? ` (${subtasks.length})` : ''}`, icon: <Stack size={16} weight="bold" />, hidden: subtasks.length === 0 && !hasSubtaskDefinitions(task) },
    { id: 'activity', label: 'Activity', icon: <Terminal size={16} weight="bold" /> },
  ];

  return (
    <div className="fixed inset-0 z-50 flex items-end sm:items-center justify-center">
      <div className="fixed inset-0 bg-black/50" onClick={onClose} />
      <div className="relative bg-gray-1 rounded-t-2xl sm:rounded-xl shadow-modal w-full sm:max-w-4xl sm:mx-6 h-[95vh] sm:h-auto sm:max-h-[85vh] flex flex-col overflow-hidden">
        {/* Mobile drag handle */}
        <div className="sm:hidden flex justify-center pt-2 pb-0 flex-shrink-0">
          <div className="w-8 h-1 rounded-full bg-gray-6" />
        </div>

        {/* Header */}
        <div className="flex items-center justify-between px-4 sm:px-6 h-12 sm:h-14 border-b border-gray-3/60 flex-shrink-0">
          <div className="flex items-center gap-2 sm:gap-3 flex-1 min-w-0 overflow-x-auto">
            <span className="text-xs text-gray-11 font-mono font-medium flex-shrink-0">KAR-{task.task_number}</span>
            <span className="text-gray-5 hidden sm:inline">/</span>
            {repoName ? (
              <span className="text-xs text-gray-9 font-mono hidden sm:inline">{repoName}</span>
            ) : (
              <span className="text-xs text-gray-9 items-center gap-1 hidden sm:flex">
                <Stack size={12} weight="bold" /> multi-repo
              </span>
            )}
            <span className="text-gray-5 hidden sm:inline">/</span>
            <select
              value={task.priority}
              onChange={(e) => handlePriorityChange(e.target.value as AgentTaskPriority)}
              className="text-xs bg-transparent text-gray-9 cursor-pointer focus:outline-none hidden sm:block"
            >
              <option value="low">Low</option>
              <option value="medium">Medium</option>
              <option value="high">High</option>
              <option value="urgent">Urgent</option>
            </select>
            <StatusBadge status={task.status} />
            {task.pr_url && (
              <a
                href={task.pr_url}
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-1 text-xs text-gray-9 hover:text-gray-12 transition-colors flex-shrink-0"
                onClick={(e) => e.stopPropagation()}
              >
                <GitPullRequest size={14} weight="bold" />
                #{task.pr_number}
                <ArrowSquareOut size={12} weight="bold" />
              </a>
            )}
          </div>

          <div className="flex items-center gap-1 sm:gap-1.5 flex-shrink-0">
            {task.status === 'failed' && (
              <button onClick={handleRetry} disabled={loading} className="h-8 px-2 sm:px-3 text-[13px] text-gray-9 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors flex items-center gap-1.5">
                <ArrowCounterClockwise size={14} weight="bold" /> <span className="hidden sm:inline">Retry</span>
              </button>
            )}
            {!['done', 'cancelled'].includes(task.status) && (
              <button onClick={handleCancel} disabled={loading} className="h-8 px-2 sm:px-3 text-[13px] text-gray-9 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors flex items-center gap-1.5">
                <Prohibit size={14} weight="bold" /> <span className="hidden sm:inline">Cancel</span>
              </button>
            )}
            <button onClick={handleDelete} className="h-8 w-8 flex items-center justify-center text-gray-8 hover:text-red-400 hover:bg-gray-3 rounded-lg transition-colors">
              <Trash size={16} weight="bold" />
            </button>
            <button onClick={onClose} className="h-8 w-8 flex items-center justify-center text-gray-8 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors">
              <X size={16} weight="bold" />
            </button>
          </div>
        </div>

        {/* Tabs — horizontally scrollable on mobile */}
        <div className="flex border-b border-gray-3/60 px-3 sm:px-6 flex-shrink-0 overflow-x-auto mobile-hide-scrollbar">
          {tabs.filter(t => !t.hidden).map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`flex items-center gap-1.5 sm:gap-2 px-2.5 sm:px-3.5 h-11 text-[13px] font-medium border-b-2 transition-colors whitespace-nowrap flex-shrink-0 ${
                activeTab === tab.id
                  ? 'border-gray-12 text-gray-12'
                  : 'border-transparent text-gray-8 hover:text-gray-11'
              }`}
            >
              {tab.icon}
              <span className="hidden sm:inline">{tab.label}</span>
            </button>
          ))}
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4 sm:p-6">
          {activeTab === 'details' && (
            <div className="space-y-4">
              <input
                defaultValue={task.title}
                onBlur={(e) => {
                  const v = e.target.value.trim();
                  if (v && v !== task.title) onUpdate(task.id, { title: v });
                }}
                className="text-xl font-semibold text-gray-12 bg-transparent w-full outline-none tracking-[-0.02em] rounded-lg px-3 py-2 -mx-3 hover:bg-gray-2 focus:bg-gray-2 transition-colors"
              />
              <div className="rounded-lg px-3 py-2 -mx-3 hover:bg-gray-2 focus-within:bg-gray-2 transition-colors">
                <MarkdownEditor
                  content={task.description || ''}
                  onSave={(md) => onUpdate(task.id, { description: md })}
                  placeholder="Add a description..."
                />
              </div>

              <TaskAttachments
                taskId={task.id}
                editable={['todo', 'plan_review', 'planning'].includes(task.status)}
              />

              {task.error_message && (
                <div className="flex items-start gap-3 p-4 rounded-lg bg-red-500/8 border border-red-500/15">
                  <WarningCircle size={16} weight="fill" className="text-red-400 mt-0.5 flex-shrink-0" />
                  <div>
                    <p className="text-[13px] font-medium text-red-400">Error</p>
                    <p className="text-[13px] text-red-300/70 mt-1">{task.error_message}</p>
                  </div>
                </div>
              )}

              {(task.cli || task.model) && (
                <div className="flex items-center gap-2.5 pt-4 border-t border-gray-3">
                  <Terminal size={14} weight="bold" className="text-gray-8" />
                  <span className="text-[13px] text-gray-9 font-medium">{task.cli || 'claude'}</span>
                  {task.model && (
                    <span className="text-[13px] text-gray-8 font-mono">{task.model}</span>
                  )}
                  {task.cost_usd > 0 && (
                    <span className="text-[13px] text-gray-8 font-mono ml-auto">${task.cost_usd.toFixed(4)}</span>
                  )}
                </div>
              )}

              <div className="text-[13px] text-gray-8 space-y-1.5 pt-4 border-t border-gray-3">
                <div className="flex items-center gap-1.5">
                  <Clock size={13} weight="bold" />
                  Created {format(new Date(task.created_at), 'MMM d, yyyy h:mm a')}
                </div>
                {task.started_at && <div>Started {formatDistanceToNow(new Date(task.started_at), { addSuffix: true })}</div>}
                {task.completed_at && <div>Completed {format(new Date(task.completed_at), 'MMM d, yyyy h:mm a')}</div>}
                {task.branch && <div className="font-mono text-gray-9">{task.branch}</div>}
              </div>
            </div>
          )}

          {activeTab === 'plan' && (
            <div className="space-y-5">
              {task.plan_content ? (
                <>
                  <MarkdownEditor
                    content={task.plan_content}
                    onSave={(md) => onUpdate(task.id, { plan_content: md })}
                    placeholder="Plan content..."
                  />
                  {task.status === 'plan_review' && (
                    <div className="flex gap-2.5 pt-4 border-t border-gray-3 sticky bottom-0 bg-gray-1 py-4">
                      <button onClick={handleApprovePlan} disabled={loading} className="h-9 px-4 text-[14px] font-medium text-white bg-green-600 hover:bg-green-500 rounded-lg transition-colors disabled:opacity-50 flex items-center gap-2">
                        <Check size={16} weight="bold" /> Approve
                      </button>
                      <button onClick={handleRejectPlan} disabled={loading} className="h-9 px-4 text-[14px] text-gray-9 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-colors disabled:opacity-50 flex items-center gap-2">
                        <X size={16} weight="bold" /> Request Changes
                      </button>
                    </div>
                  )}
                </>
              ) : (
                <div className="flex flex-col items-center justify-center py-20 text-gray-8">
                  <Article size={32} weight="thin" className="mb-3" />
                  <p className="text-[14px]">No plan yet</p>
                  {task.status === 'planning' && (
                    <p className="text-[13px] mt-1.5 text-sun-9 flex items-center gap-1.5">
                      <Lightning size={14} weight="fill" className="animate-pulse" />
                      Agent is planning...
                    </p>
                  )}
                </div>
              )}
            </div>
          )}

          {activeTab === 'subtasks' && (
            <div className="space-y-px">
              {subtasks.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-20 text-gray-8">
                  <Stack size={32} weight="thin" className="mb-3" />
                  <p className="text-[14px]">No subtasks</p>
                  {task.status === 'plan_review' && hasSubtaskDefinitions(task) && (
                    <p className="text-[13px] mt-1.5 text-gray-9">Approve the plan to create subtasks</p>
                  )}
                </div>
              ) : (
                <>
                  <div className="flex items-center gap-3 mb-4">
                    <span className="text-[14px] text-gray-9">
                      {subtasks.filter(s => s.status === 'done').length} of {subtasks.length} complete
                    </span>
                    <div className="flex-1 h-1.5 bg-gray-3 rounded-full overflow-hidden">
                      <div
                        className="h-full bg-green-500 rounded-full transition-all duration-300"
                        style={{ width: `${subtasks.length > 0 ? (subtasks.filter(s => s.status === 'done').length / subtasks.length) * 100 : 0}%` }}
                      />
                    </div>
                  </div>
                  {subtasks.map((sub) => <SubtaskRow key={sub.id} task={sub} />)}
                </>
              )}
            </div>
          )}

          {activeTab === 'activity' && (
            <div className="flex flex-col h-full">
              {logsLoading && logs.length === 0 ? (
                <div className="flex items-center justify-center py-20 text-gray-8 gap-2 text-[14px]">
                  <Lightning size={18} weight="fill" className="animate-pulse" /> Loading...
                </div>
              ) : logs.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-20 text-gray-8">
                  <Terminal size={32} weight="thin" className="mb-3" />
                  <p className="text-[14px]">No activity yet</p>
                </div>
              ) : (
                <div className="font-mono text-[12px] sm:text-[13px] leading-[1.6] sm:leading-[1.8] bg-gray-2 border border-gray-4 rounded-lg p-3 sm:p-4 overflow-x-auto mb-4">
                  {logs.map((log) => <LogLine key={log.id} log={log} />)}
                  <div ref={logsEndRef} />
                </div>
              )}

              {/* Comment input */}
              <div className="mt-auto pt-3 border-t border-gray-3">
                <div className="flex gap-2 items-end">
                  <textarea
                    ref={commentRef}
                    value={comment}
                    onChange={(e) => setComment(e.target.value)}
                    onKeyDown={(e) => { if (e.key === 'Enter' && e.metaKey) handlePostComment(); }}
                    rows={2}
                    placeholder="Leave a comment..."
                    className="flex-1 px-3 py-2 rounded-lg bg-gray-2 border border-gray-4 text-gray-11 text-[16px] sm:text-[13px] focus:outline-none focus:border-gray-6 placeholder:text-gray-7 resize-none"
                  />
                  <button
                    onClick={handlePostComment}
                    disabled={loading || !comment.trim()}
                    className="h-9 px-3.5 text-[13px] font-medium text-white bg-sun-9 hover:bg-sun-10 text-gray-1 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex-shrink-0"
                  >
                    {task.status === 'review' ? 'Request Changes' : task.status === 'plan_review' ? 'Request Changes' : 'Comment'}
                  </button>
                </div>
                {(task.status === 'review' || task.status === 'plan_review') && (
                  <p className="text-[11px] text-gray-7 mt-1.5">
                    Commenting will send feedback to the agent. <span className="text-gray-8">⌘ Enter</span> to submit.
                  </p>
                )}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function StatusBadge({ status }: { status: AgentTaskStatus }) {
  const config: Record<AgentTaskStatus, { label: string; color: string }> = {
    todo:         { label: 'Todo',        color: '#a09e97' },
    planning:     { label: 'Planning',    color: '#e5b847' },
    plan_review:  { label: 'Plan Review', color: '#e5b847' },
    in_progress:  { label: 'In Progress', color: '#e5b847' },
    review:       { label: 'Review',      color: '#60a5a0' },
    done:         { label: 'Done',        color: '#6ab070' },
    failed:       { label: 'Failed',      color: '#d4583a' },
    cancelled:    { label: 'Cancelled',   color: '#82807a' },
  };

  const c = config[status];
  return (
    <span className="inline-flex items-center gap-1.5 text-xs font-medium text-gray-9">
      <span className="w-2 h-2 rounded-full" style={{ backgroundColor: c.color }} />
      {c.label}
    </span>
  );
}

const logColors: Record<string, string> = {
  info: 'text-gray-10',
  error: 'text-red-400',
  command: 'text-blue-400',
  output: 'text-gray-8',
  claude: 'text-sun-10',
  tool: 'text-sun-8',
  comment: 'text-gray-12',
};

function LogLine({ log }: { log: AgentLog }) {
  const time = format(new Date(log.created_at), 'HH:mm:ss');

  if (log.log_type === 'comment') {
    return (
      <div className="my-1.5 rounded-lg bg-sun-3 border border-sun-5 px-3 py-2">
        <div className="flex items-center gap-2 mb-1">
          <ChatText size={13} weight="bold" className="text-sun-9 flex-shrink-0" />
          <span className="text-[11px] text-sun-9 font-medium">You</span>
          <span className="text-[11px] text-gray-7 ml-auto">{time}</span>
        </div>
        <p className="text-[13px] text-gray-12 whitespace-pre-wrap break-words font-sans">{log.message}</p>
      </div>
    );
  }

  const color = logColors[log.log_type] || 'text-gray-10';

  return (
    <div className="flex flex-col sm:flex-row gap-0.5 sm:gap-3 hover:bg-gray-3 px-1.5 -mx-1.5 rounded-sm py-0.5 sm:py-0">
      <div className="flex gap-2 sm:gap-3 flex-shrink-0">
        <span className="text-gray-7 flex-shrink-0 select-none">{time}</span>
        <span className="text-gray-7 flex-shrink-0 w-14 truncate">{log.phase}</span>
      </div>
      <span className={`${color} break-all`}>{log.message}</span>
    </div>
  );
}

const subtaskColors: Record<string, string> = {
  todo:        '#a09e97',
  planning:    '#e5b847',
  plan_review: '#e5b847',
  in_progress: '#e5b847',
  review:      '#60a5a0',
  done:        '#6ab070',
  failed:      '#d4583a',
  cancelled:   '#82807a',
};

const subtaskLabels: Record<string, string> = {
  todo: 'Todo', planning: 'Planning', plan_review: 'Plan Review',
  in_progress: 'Working', review: 'Review', done: 'Done', failed: 'Failed',
  cancelled: 'Cancelled',
};

function SubtaskRow({ task }: { task: AgentTask }) {
  const color = subtaskColors[task.status] || '#b4b4bf';
  const repoName = task.repo ? (task.repo.split('/').pop() || task.repo) : '\u2014';

  return (
    <div className="flex flex-col sm:flex-row sm:items-center gap-1 sm:gap-3 px-3 sm:px-4 py-2.5 sm:py-0 sm:h-11 rounded-lg hover:bg-gray-3 transition-colors">
      <div className="flex items-center gap-2 sm:gap-3 min-w-0">
        <span
          className={`w-2.5 h-2.5 rounded-full flex-shrink-0 ${(task.status === 'planning' || task.status === 'in_progress') ? 'animate-pulse' : ''}`}
          style={{ backgroundColor: color }}
        />
        <span className="text-xs text-gray-7 font-mono flex-shrink-0">KAR-{task.task_number}</span>
        <span className="text-[14px] text-gray-12 truncate flex-1">{task.title}</span>
      </div>
      <div className="flex items-center gap-2 sm:gap-3 ml-[18px] sm:ml-0 flex-shrink-0">
        <span className="text-xs text-gray-8 font-mono">{repoName}</span>
        <span className="text-xs" style={{ color }}>{subtaskLabels[task.status]}</span>
        {task.pr_url && (
          <a href={task.pr_url} target="_blank" rel="noopener noreferrer"
            className="text-xs text-gray-8 hover:text-gray-12 flex items-center gap-1 transition-colors"
            onClick={(e) => e.stopPropagation()}>
            <GitPullRequest size={13} weight="bold" /> #{task.pr_number}
          </a>
        )}
      </div>
    </div>
  );
}
