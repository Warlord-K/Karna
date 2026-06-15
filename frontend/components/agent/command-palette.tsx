'use client';

import { createContext, useContext, useEffect, useMemo, useState } from 'react';
import { useRouter } from 'next/navigation';
import { Command } from 'cmdk';
import {
  Kanban,
  ChatCenteredText,
  CalendarBlank,
  GitFork,
  Robot,
  ShieldCheck,
  Plus,
  PaperPlaneTilt,
  MagnifyingGlass,
  ArrowRight,
} from '@phosphor-icons/react';
import { useTasks } from '@/hooks/use-tasks';
import { getTaskLabel, getTaskTitle } from '@/lib/agent-tasks';
import { Kbd } from '@/components/ui/kbd';

interface CommandPaletteContextValue {
  open: boolean;
  setOpen: (open: boolean) => void;
  toggle: () => void;
}

const CommandPaletteContext = createContext<CommandPaletteContextValue | null>(null);

export function useCommandPalette(): CommandPaletteContextValue {
  const ctx = useContext(CommandPaletteContext);
  if (!ctx) throw new Error('useCommandPalette must be used within CommandPaletteProvider');
  return ctx;
}

export function CommandPaletteProvider({
  isReady,
  children,
}: {
  isReady: boolean;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setOpen((v) => !v);
      }
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, []);

  const value = useMemo<CommandPaletteContextValue>(
    () => ({ open, setOpen, toggle: () => setOpen((v) => !v) }),
    [open],
  );

  return (
    <CommandPaletteContext.Provider value={value}>
      {children}
      <CommandPalette isReady={isReady} open={open} onOpenChange={setOpen} />
    </CommandPaletteContext.Provider>
  );
}

const NAV_TARGETS = [
  { href: '/', label: 'Go to Board', icon: Kanban },
  { href: '/chat', label: 'Go to Chat', icon: ChatCenteredText },
  { href: '/schedules', label: 'Go to Schedules', icon: CalendarBlank },
  { href: '/repos', label: 'Go to Repos', icon: GitFork },
  { href: '/agents', label: 'Go to Agents', icon: Robot },
  { href: '/policies', label: 'Go to Policies', icon: ShieldCheck },
] as const;

function CommandPalette({
  isReady,
  open,
  onOpenChange,
}: {
  isReady: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const router = useRouter();
  const [search, setSearch] = useState('');
  const { data: tasks = [] } = useTasks(isReady && open);

  // Reset the query whenever the palette closes so it always opens fresh.
  useEffect(() => {
    if (!open) setSearch('');
  }, [open]);

  // Lock background scroll while open.
  useEffect(() => {
    if (!open) return;
    const prev = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    return () => {
      document.body.style.overflow = prev;
    };
  }, [open]);

  if (!open) return null;

  const go = (href: string) => {
    onOpenChange(false);
    router.push(href);
  };

  const itemClass =
    'flex items-center gap-2.5 px-2.5 h-9 rounded-lg text-[13px] text-gray-10 cursor-pointer select-none ' +
    'transition-smooth data-[selected=true]:bg-gray-3 data-[selected=true]:text-gray-12 ' +
    '[&_svg]:text-gray-8 data-[selected=true]:[&_svg]:text-sun-10 ' +
    'data-[selected=true]:[&_.cmd-arrow]:opacity-100';

  const taskMatches = tasks.slice(0, 200);

  return (
    <div className="fixed inset-0 z-[100] flex items-start justify-center p-4 sm:pt-[12vh]">
      <div
        className="fixed inset-0 bg-black/55 backdrop-blur-[2px] animate-fade-in"
        onClick={() => onOpenChange(false)}
      />
      <div className="relative w-full max-w-[560px] animate-scale-in origin-top">
        <Command
          label="Command palette"
          className="overflow-hidden rounded-xl border border-gray-4 bg-popover shadow-modal"
          onKeyDown={(e) => {
            if (e.key === 'Escape') {
              e.preventDefault();
              onOpenChange(false);
            }
          }}
          filter={(value, search) => {
            if (!search) return 1;
            return value.toLowerCase().includes(search.toLowerCase()) ? 1 : 0;
          }}
        >
          <div className="flex items-center gap-2.5 px-3.5 border-b border-gray-3">
            <MagnifyingGlass size={16} weight="bold" className="text-gray-8 flex-shrink-0" />
            <Command.Input
              autoFocus
              value={search}
              onValueChange={setSearch}
              placeholder="Search tasks or jump to a page..."
              className="h-12 flex-1 bg-transparent text-[14px] text-gray-12 placeholder:text-gray-7 focus:outline-none"
            />
            <Kbd>Esc</Kbd>
          </div>

          <Command.List className="max-h-[min(60vh,420px)] overflow-y-auto p-1.5">
            <Command.Empty className="py-8 text-center text-[13px] text-gray-8">
              No results found.
            </Command.Empty>

            <Command.Group
              heading="Create"
              className="[&_[cmdk-group-heading]]:px-2.5 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-wider [&_[cmdk-group-heading]]:text-gray-7"
            >
              <Command.Item value="new task create" onSelect={() => go('/tasks/new')} className={itemClass}>
                <Plus size={15} weight="bold" />
                <span>New task</span>
              </Command.Item>
              <Command.Item value="new chat create conversation" onSelect={() => go('/chat')} className={itemClass}>
                <PaperPlaneTilt size={15} weight="bold" />
                <span>New chat</span>
              </Command.Item>
            </Command.Group>

            <Command.Group
              heading="Navigate"
              className="[&_[cmdk-group-heading]]:px-2.5 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-wider [&_[cmdk-group-heading]]:text-gray-7 mt-1"
            >
              {NAV_TARGETS.map(({ href, label, icon: Icon }) => (
                <Command.Item key={href} value={label} onSelect={() => go(href)} className={itemClass}>
                  <Icon size={15} weight="bold" />
                  <span>{label}</span>
                </Command.Item>
              ))}
            </Command.Group>

            {taskMatches.length > 0 && (
              <Command.Group
                heading="Tasks"
                className="[&_[cmdk-group-heading]]:px-2.5 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-wider [&_[cmdk-group-heading]]:text-gray-7 mt-1"
              >
                {taskMatches.map((task) => (
                  <Command.Item
                    key={task.id}
                    value={`${getTaskLabel(task)} ${getTaskTitle(task)} ${task.id}`}
                    onSelect={() => go(`/tasks/${task.id}`)}
                    className={itemClass}
                  >
                    <span className="font-mono text-[11px] text-gray-7 w-[58px] flex-shrink-0 truncate">
                      {getTaskLabel(task)}
                    </span>
                    <span className="flex-1 truncate text-gray-11">{getTaskTitle(task)}</span>
                    <ArrowRight size={13} weight="bold" className="cmd-arrow flex-shrink-0 opacity-0 transition-smooth" />
                  </Command.Item>
                ))}
              </Command.Group>
            )}
          </Command.List>
        </Command>
      </div>
    </div>
  );
}
