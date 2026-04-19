'use client';

import { useSession, signOut } from 'next-auth/react';
import { usePathname } from 'next/navigation';
import Link from 'next/link';
import { useTasks } from '@/hooks/use-tasks';
import { AuthDisabledProvider } from '@/lib/auth-context';
import { SignOut, CircleNotch, CalendarBlank, Kanban, GitFork } from '@phosphor-icons/react';
import { Toaster } from 'react-hot-toast';

const NAV_ITEMS = [
  { href: '/', label: 'Board', icon: Kanban },
  { href: '/schedules', label: 'Schedules', icon: CalendarBlank },
  { href: '/repos', label: 'Repos', icon: GitFork },
] as const;

export function DashboardShell({ authDisabled, children }: { authDisabled: boolean; children: React.ReactNode }) {
  const { data: session, status: authStatus } = useSession();
  const isReady = authDisabled || authStatus === 'authenticated';
  const pathname = usePathname();

  const { data: tasks = [], isLoading } = useTasks(isReady);

  const activeTasks = tasks.filter(t =>
    (t.status === 'planning' || t.status === 'in_progress') && !(t.subtask_count && t.subtask_count > 0)
  );

  if ((!authDisabled && authStatus === 'loading') || isLoading) {
    return (
      <div className="h-screen flex items-center justify-center bg-background">
        <CircleNotch size={24} weight="bold" className="text-gray-8 animate-spin" />
      </div>
    );
  }

  const activeSection = pathname === '/' ? '/'
    : pathname.startsWith('/schedules') ? '/schedules'
    : pathname.startsWith('/repos') ? '/repos'
    : pathname.startsWith('/tasks') ? '/'
    : '/';

  return (
    <AuthDisabledProvider value={authDisabled}>
      <div className="h-screen flex flex-col bg-background">
        <Toaster
          position="top-right"
          toastOptions={{
            style: {
              background: '#18181b',
              color: '#ededef',
              border: '1px solid #26262b',
              borderRadius: '8px',
              fontSize: '14px',
              padding: '12px 16px',
              boxShadow: '0 4px 12px rgba(0,0,0,0.3)',
            },
          }}
        />

        <header className="flex-shrink-0 bg-gray-1 shadow-card z-10 relative">
          <div className="flex items-center justify-between h-14 px-3 sm:px-6">
            <div className="flex items-center gap-2 sm:gap-4">
              <Link href="/" className="flex items-center gap-2 sm:gap-2.5">
                <img src="/logo-192.png" alt="Karna" width={20} height={20} />
                <span className="text-[15px] font-semibold text-gray-12 tracking-[-0.01em]">Karna</span>
              </Link>

              <div className="flex items-center bg-gray-2 rounded-lg p-0.5 border border-gray-3">
                {NAV_ITEMS.map(({ href, label, icon: Icon }) => (
                  <Link
                    key={href}
                    href={href}
                    className={`flex items-center gap-1.5 px-2.5 h-7 rounded-md text-[12px] font-medium transition-all duration-200 ${
                      activeSection === href
                        ? 'bg-gray-4 text-gray-12 shadow-[0_0_8px_hsl(40_90%_56%/0.08)]'
                        : 'text-gray-8 hover:text-gray-11 hover:bg-gray-3'
                    }`}
                  >
                    <Icon size={13} weight="bold" className={activeSection === href ? 'text-sun-10' : ''} />
                    <span className="hidden sm:inline">{label}</span>
                  </Link>
                ))}
              </div>

              {activeTasks.length > 0 && (
                <div className="flex items-center gap-2 text-[13px] text-gray-9">
                  <span className="w-2 h-2 rounded-full bg-sun-9 animate-pulse" />
                  <span className="hidden sm:inline">{activeTasks.length} running</span>
                </div>
              )}
            </div>

            <div className="flex items-center gap-1 sm:gap-1.5">
              {session?.user?.image && (
                <img src={session.user.image} alt="" className="w-7 h-7 rounded-full mr-0.5 sm:mr-1 hidden sm:block" />
              )}
              {!authDisabled && (
                <button
                  onClick={() => signOut()}
                  className="h-8 w-8 flex items-center justify-center text-gray-8 hover:text-gray-11 hover:bg-gray-3 rounded-lg transition-colors ml-0.5"
                >
                  <SignOut size={16} weight="bold" />
                </button>
              )}
            </div>
          </div>
        </header>

        <main className="flex-1 overflow-hidden relative">
          {/* Decorative background */}
          <div className="board-bg-decoration" aria-hidden="true">
            <div className="board-diamond board-diamond--1" />
            <div className="board-diamond board-diamond--2" />
            <div className="board-diamond board-diamond--3" />
            <div className="board-diamond board-diamond--4" />
          </div>
          <div className="relative z-[1] h-full">
            {children}
          </div>
        </main>
      </div>
    </AuthDisabledProvider>
  );
}
