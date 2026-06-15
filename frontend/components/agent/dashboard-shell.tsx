'use client';

import { useEffect, useState } from 'react';
import { useSession, signOut } from 'next-auth/react';
import { usePathname } from 'next/navigation';
import Link from 'next/link';
import {
  SignOut,
  CircleNotch,
  CalendarBlank,
  Kanban,
  GitFork,
  Robot,
  ShieldCheck,
  ChatCenteredText,
  MagnifyingGlass,
  List as ListIcon,
  X,
  SidebarSimple,
  type Icon,
} from '@phosphor-icons/react';
import { Toaster } from 'react-hot-toast';
import { useTasks } from '@/hooks/use-tasks';
import { AuthDisabledProvider } from '@/lib/auth-context';
import { cn } from '@/lib/utils';
import { Kbd } from '@/components/ui/kbd';
import { Tooltip } from '@/components/ui/tooltip';
import { CommandPaletteProvider, useCommandPalette } from '@/components/agent/command-palette';

interface NavItem {
  href: string;
  label: string;
  icon: Icon;
}

interface NavSection {
  label?: string;
  items: NavItem[];
}

const NAV_SECTIONS: NavSection[] = [
  {
    items: [
      { href: '/', label: 'Board', icon: Kanban },
      { href: '/chat', label: 'Chat', icon: ChatCenteredText },
    ],
  },
  {
    label: 'Configure',
    items: [
      { href: '/schedules', label: 'Schedules', icon: CalendarBlank },
      { href: '/repos', label: 'Repos', icon: GitFork },
      { href: '/agents', label: 'Agents', icon: Robot },
      { href: '/policies', label: 'Policies', icon: ShieldCheck },
    ],
  },
];

const SECTION_TITLES: Record<string, string> = {
  '/': 'Board',
  '/chat': 'Chat',
  '/schedules': 'Schedules',
  '/repos': 'Repos',
  '/agents': 'Agents',
  '/policies': 'Policies',
};

function activeSectionFor(pathname: string): string {
  if (pathname === '/') return '/';
  if (pathname.startsWith('/chat')) return '/chat';
  if (pathname.startsWith('/schedules')) return '/schedules';
  if (pathname.startsWith('/repos')) return '/repos';
  if (pathname.startsWith('/agents')) return '/agents';
  if (pathname.startsWith('/policies')) return '/policies';
  if (pathname.startsWith('/tasks')) return '/';
  return '/';
}

export function DashboardShell({ authDisabled, children }: { authDisabled: boolean; children: React.ReactNode }) {
  const { status: authStatus } = useSession();
  const isReady = authDisabled || authStatus === 'authenticated';

  const { isLoading } = useTasks(isReady);

  if ((!authDisabled && authStatus === 'loading') || isLoading) {
    return (
      <div className="h-screen flex items-center justify-center bg-background">
        <CircleNotch size={24} weight="bold" className="text-gray-8 animate-spin" />
      </div>
    );
  }

  return (
    <AuthDisabledProvider value={authDisabled}>
      <CommandPaletteProvider isReady={isReady}>
        <ShellInner authDisabled={authDisabled} isReady={isReady}>
          {children}
        </ShellInner>
      </CommandPaletteProvider>
    </AuthDisabledProvider>
  );
}

function ShellInner({
  authDisabled,
  isReady,
  children,
}: {
  authDisabled: boolean;
  isReady: boolean;
  children: React.ReactNode;
}) {
  const { data: session } = useSession();
  const pathname = usePathname();
  const { toggle: togglePalette } = useCommandPalette();

  const [collapsed, setCollapsed] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);

  // Restore the persisted collapse preference once mounted.
  useEffect(() => {
    setCollapsed(localStorage.getItem('karna:sidebar-collapsed') === '1');
  }, []);

  useEffect(() => {
    setMobileOpen(false);
  }, [pathname]);

  const setCollapsedPersisted = (value: boolean) => {
    setCollapsed(value);
    localStorage.setItem('karna:sidebar-collapsed', value ? '1' : '0');
  };

  const { data: tasks = [] } = useTasks(isReady);
  const activeCount = tasks.filter(
    (t) => (t.status === 'planning' || t.status === 'in_progress') && !(t.subtask_count && t.subtask_count > 0),
  ).length;

  const activeSection = activeSectionFor(pathname);

  return (
    <div className="h-screen flex bg-background overflow-hidden">
      <Toaster
        position="top-right"
        toastOptions={{
          style: {
            background: '#1a1917',
            color: '#edece8',
            border: '1px solid #2a2926',
            borderRadius: '10px',
            fontSize: '13px',
            padding: '10px 14px',
            boxShadow: '0 8px 30px rgba(0,0,0,0.4)',
          },
        }}
      />

      {/* Desktop sidebar */}
      <aside
        className={cn(
          'hidden sm:flex flex-col flex-shrink-0 border-r border-gray-3 bg-gray-1 transition-[width] duration-200 ease-out',
          collapsed ? 'w-[60px]' : 'w-[228px]',
        )}
      >
        <SidebarContent
          collapsed={collapsed}
          activeSection={activeSection}
          activeCount={activeCount}
          onPalette={togglePalette}
          onToggleCollapse={() => setCollapsedPersisted(!collapsed)}
          session={session}
          authDisabled={authDisabled}
        />
      </aside>

      {/* Mobile drawer */}
      {mobileOpen && (
        <div className="sm:hidden fixed inset-0 z-50">
          <div className="absolute inset-0 bg-black/55 animate-fade-in" onClick={() => setMobileOpen(false)} />
          <aside className="absolute inset-y-0 left-0 w-[240px] bg-gray-1 border-r border-gray-3 flex flex-col animate-fade-in-up">
            <SidebarContent
              collapsed={false}
              activeSection={activeSection}
              activeCount={activeCount}
              onPalette={() => {
                setMobileOpen(false);
                togglePalette();
              }}
              onClose={() => setMobileOpen(false)}
              session={session}
              authDisabled={authDisabled}
            />
          </aside>
        </div>
      )}

      <div className="flex-1 flex flex-col min-w-0">
        {/* Slim top bar */}
        <header className="flex-shrink-0 h-12 border-b border-gray-3 flex items-center gap-2 px-3 sm:px-5">
          <button
            onClick={() => setMobileOpen(true)}
            className="sm:hidden h-8 w-8 flex items-center justify-center text-gray-9 hover:text-gray-12 hover:bg-gray-3 rounded-lg transition-smooth focus-ring"
            aria-label="Open navigation"
          >
            <ListIcon size={18} weight="bold" />
          </button>

          <h1 className="text-[13px] font-medium text-gray-12 tracking-[-0.01em]">
            {SECTION_TITLES[activeSection] ?? 'Board'}
          </h1>

          <button
            onClick={togglePalette}
            className="ml-auto group flex items-center gap-2 h-8 pl-2.5 pr-2 rounded-lg border border-gray-3 bg-gray-2 text-gray-8 hover:text-gray-11 hover:border-gray-4 transition-smooth focus-ring"
          >
            <MagnifyingGlass size={14} weight="bold" />
            <span className="hidden md:inline text-[12px]">Search</span>
            <span className="hidden md:flex items-center gap-0.5">
              <Kbd>⌘</Kbd>
              <Kbd>K</Kbd>
            </span>
          </button>

          {activeCount > 0 && (
            <div className="flex items-center gap-1.5 text-[12px] text-gray-9">
              <span className="w-1.5 h-1.5 rounded-full bg-sun-9 animate-pulse" />
              <span className="hidden sm:inline tabular-nums">{activeCount} running</span>
            </div>
          )}
        </header>

        <main className="flex-1 overflow-hidden relative">
          <div className="board-bg-decoration" aria-hidden="true" />
          <div className="relative z-[1] h-full animate-fade-in">{children}</div>
        </main>
      </div>
    </div>
  );
}

function SidebarContent({
  collapsed,
  activeSection,
  activeCount,
  onPalette,
  onToggleCollapse,
  onClose,
  session,
  authDisabled,
}: {
  collapsed: boolean;
  activeSection: string;
  activeCount: number;
  onPalette: () => void;
  onToggleCollapse?: () => void;
  onClose?: () => void;
  session: ReturnType<typeof useSession>['data'];
  authDisabled: boolean;
}) {
  return (
    <>
      {/* Brand + collapse / close */}
      <div className={cn('flex items-center h-12 flex-shrink-0', collapsed ? 'justify-center px-0' : 'px-3')}>
        <Link href="/" className="flex items-center gap-2 min-w-0">
          <img src="/logo-192.png" alt="Karna" width={20} height={20} className="flex-shrink-0" />
          {!collapsed && (
            <span className="text-[15px] font-semibold text-gray-12 tracking-[-0.02em] truncate">Karna</span>
          )}
        </Link>
        {!collapsed && onToggleCollapse && (
          <button
            onClick={onToggleCollapse}
            className="ml-auto h-7 w-7 flex items-center justify-center text-gray-7 hover:text-gray-11 hover:bg-gray-3 rounded-md transition-smooth focus-ring"
            aria-label="Collapse sidebar"
          >
            <SidebarSimple size={16} weight="bold" />
          </button>
        )}
        {!collapsed && onClose && (
          <button
            onClick={onClose}
            className="ml-auto h-7 w-7 flex items-center justify-center text-gray-7 hover:text-gray-11 hover:bg-gray-3 rounded-md transition-smooth focus-ring"
            aria-label="Close navigation"
          >
            <X size={16} weight="bold" />
          </button>
        )}
      </div>

      {/* Search trigger */}
      <div className={cn('flex-shrink-0', collapsed ? 'px-2 pb-2' : 'px-2 pb-2')}>
        {collapsed ? (
          <Tooltip label="Search  ⌘K" side="right">
            <button
              onClick={onPalette}
              className="h-9 w-9 flex items-center justify-center text-gray-8 hover:text-gray-11 hover:bg-gray-3 rounded-lg transition-smooth focus-ring"
              aria-label="Search"
            >
              <MagnifyingGlass size={16} weight="bold" />
            </button>
          </Tooltip>
        ) : (
          <button
            onClick={onPalette}
            className="w-full group flex items-center gap-2 h-9 px-2.5 rounded-lg border border-gray-3 bg-gray-2 text-gray-8 hover:text-gray-11 hover:border-gray-4 transition-smooth focus-ring"
          >
            <MagnifyingGlass size={15} weight="bold" />
            <span className="text-[13px]">Search…</span>
            <span className="ml-auto flex items-center gap-0.5">
              <Kbd>⌘</Kbd>
              <Kbd>K</Kbd>
            </span>
          </button>
        )}
      </div>

      {/* Nav */}
      <nav className="flex-1 overflow-y-auto px-2 space-y-4 py-1">
        {NAV_SECTIONS.map((section, idx) => (
          <div key={section.label ?? idx} className={cn('space-y-0.5', collapsed && 'flex flex-col items-center')}>
            {section.label && !collapsed && (
              <p className="px-2.5 pt-2 pb-1 text-[10px] font-medium uppercase tracking-wider text-gray-7">
                {section.label}
              </p>
            )}
            {section.label && collapsed && idx > 0 && <div className="my-2 h-px w-7 bg-gray-3" />}
            {section.items.map((item) => (
              <NavLink
                key={item.href}
                item={item}
                active={activeSection === item.href}
                collapsed={collapsed}
                badge={item.href === '/' && activeCount > 0 ? activeCount : undefined}
              />
            ))}
          </div>
        ))}
      </nav>

      {/* User footer */}
      <div className={cn('flex-shrink-0 border-t border-gray-3 p-2', collapsed && 'flex justify-center')}>
        {collapsed ? (
          !authDisabled ? (
            <Tooltip label="Sign out" side="right">
              <button
                onClick={() => signOut()}
                className="h-9 w-9 flex items-center justify-center text-gray-8 hover:text-gray-11 hover:bg-gray-3 rounded-lg transition-smooth focus-ring"
                aria-label="Sign out"
              >
                <SignOut size={16} weight="bold" />
              </button>
            </Tooltip>
          ) : (
            session?.user?.image && (
              <img src={session.user.image} alt="" className="w-7 h-7 rounded-full" />
            )
          )
        ) : (
          <div className="flex items-center gap-2 px-1.5 h-9">
            {session?.user?.image ? (
              <img src={session.user.image} alt="" className="w-6 h-6 rounded-full flex-shrink-0" />
            ) : (
              <div className="w-6 h-6 rounded-full bg-gray-4 flex items-center justify-center text-gray-9 text-[11px] font-medium flex-shrink-0">
                {(session?.user?.name ?? session?.user?.email ?? 'K').slice(0, 1).toUpperCase()}
              </div>
            )}
            <span className="flex-1 min-w-0 text-[12px] text-gray-10 truncate">
              {session?.user?.name ?? session?.user?.email ?? 'Workspace'}
            </span>
            {!authDisabled && (
              <button
                onClick={() => signOut()}
                className="h-7 w-7 flex items-center justify-center text-gray-7 hover:text-gray-11 hover:bg-gray-3 rounded-md transition-smooth focus-ring flex-shrink-0"
                aria-label="Sign out"
              >
                <SignOut size={15} weight="bold" />
              </button>
            )}
          </div>
        )}
      </div>
    </>
  );
}

function NavLink({
  item,
  active,
  collapsed,
  badge,
}: {
  item: NavItem;
  active: boolean;
  collapsed: boolean;
  badge?: number;
}) {
  const Icon = item.icon;
  const link = (
    <Link
      href={item.href}
      className={cn(
        'relative flex items-center h-9 rounded-lg text-[13px] font-medium transition-smooth focus-ring',
        collapsed ? 'justify-center w-9 mx-auto' : 'gap-2.5 px-2.5',
        active ? 'bg-gray-3 text-gray-12' : 'text-gray-9 hover:text-gray-12 hover:bg-gray-2',
      )}
      aria-current={active ? 'page' : undefined}
    >
      {active && (
        <span className="absolute left-0 top-1/2 -translate-y-1/2 h-4 w-[2.5px] rounded-full bg-sun-9" />
      )}
      <Icon size={16} weight={active ? 'fill' : 'regular'} className={active ? 'text-sun-10' : ''} />
      {!collapsed && <span className="truncate">{item.label}</span>}
      {!collapsed && badge !== undefined && (
        <span className="ml-auto text-[11px] tabular-nums text-gray-8">{badge}</span>
      )}
    </Link>
  );

  if (collapsed) {
    return (
      <Tooltip label={item.label} side="right">
        {link}
      </Tooltip>
    );
  }
  return link;
}
