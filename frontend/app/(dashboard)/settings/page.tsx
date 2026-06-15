'use client';

import { useEffect, useMemo, useRef, useState } from 'react';
import { useSession } from 'next-auth/react';
import { useQuery } from '@tanstack/react-query';
import { Check, Copy, Info } from '@phosphor-icons/react';
import { fetchUsers, type UserSummary } from '@/lib/agent-tasks';
import { useAuthDisabled } from '@/lib/auth-context';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { PageHeader } from '@/components/ui/page-header';

const USERS_QUERY_KEY = ['users'] as const;

function normalizeEmail(email?: string | null): string {
  return email?.trim().toLowerCase() ?? '';
}

function CopyValueButton({ value, label }: { value: string | null; label: string }) {
  const [copied, setCopied] = useState(false);
  const timeoutRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (timeoutRef.current !== null) {
        window.clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  const handleCopy = async () => {
    if (!value) return;
    await navigator.clipboard.writeText(value);
    setCopied(true);
    if (timeoutRef.current !== null) {
      window.clearTimeout(timeoutRef.current);
    }
    timeoutRef.current = window.setTimeout(() => setCopied(false), 1500);
  };

  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      onClick={handleCopy}
      disabled={!value}
      aria-label={copied ? `${label} copied` : `Copy ${label}`}
      title={copied ? 'Copied' : `Copy ${label}`}
    >
      {copied ? <Check size={14} weight="bold" className="text-sun-10" /> : <Copy size={14} weight="bold" />}
    </Button>
  );
}

export default function SettingsPage() {
  const authDisabled = useAuthDisabled();
  const { data: session, status: authStatus } = useSession();
  const isReady = authDisabled || authStatus === 'authenticated';

  const { data: users = [], isLoading } = useQuery<UserSummary[]>({
    queryKey: USERS_QUERY_KEY,
    queryFn: ({ signal }) => fetchUsers(signal),
    enabled: isReady,
    staleTime: 60_000,
  });

  const sessionUser = session?.user as { id?: string; name?: string | null; email?: string | null } | undefined;
  const sessionUserId = sessionUser?.id ?? null;
  const sessionEmail = normalizeEmail(sessionUser?.email);

  const userMatchedByEmail = useMemo(() => {
    if (!sessionEmail) return null;
    return users.find((user) => normalizeEmail(user.email) === sessionEmail) ?? null;
  }, [users, sessionEmail]);

  const currentUserId = sessionUserId ?? userMatchedByEmail?.id ?? null;
  const currentUserName = sessionUser?.name?.trim() || userMatchedByEmail?.name?.trim() || 'Unknown user';
  const currentUserEmail = sessionUser?.email?.trim() || userMatchedByEmail?.email?.trim() || 'No email';
  const resolvedViaEmail = !sessionUserId && !!userMatchedByEmail?.id;

  const isCurrentUser = (user: UserSummary): boolean => {
    if (currentUserId) return user.id === currentUserId;
    if (!sessionEmail) return false;
    return normalizeEmail(user.email) === sessionEmail;
  };

  return (
    <div className="h-full overflow-y-auto">
      <div className="w-full px-4 sm:px-6 lg:px-8 py-6">
        <PageHeader
          title="Settings"
          description="Copy Karna user IDs here when building Slack user_map entries."
        />

        <div className="max-w-5xl space-y-6">
          <section className="space-y-3">
            <h2 className="text-[13px] font-medium uppercase tracking-wider text-gray-8">Your account</h2>
            <Card className="p-4 sm:p-5 space-y-3">
              <div className="flex items-center gap-2">
                <span className="text-[15px] font-medium text-gray-12 tracking-[-0.01em]">{currentUserName}</span>
                <Badge tone="accent">You</Badge>
              </div>

              <div className="text-[13px] text-gray-8">{currentUserEmail}</div>

              <div className="flex items-start justify-between gap-3 rounded-lg border border-gray-3 bg-gray-1/60 px-3 py-2.5">
                <div className="min-w-0">
                  <p className="text-[11px] uppercase tracking-wider text-gray-7">Karna user ID</p>
                  <p className="font-mono text-[12px] text-gray-11 break-all mt-1">{currentUserId ?? 'Unavailable'}</p>
                </div>
                <CopyValueButton value={currentUserId} label="Karna user ID" />
              </div>

              {resolvedViaEmail && (
                <p className="text-[12px] text-gray-8">
                  Session did not include your ID, so it was resolved by matching your session email to the users list.
                </p>
              )}
            </Card>
          </section>

          <section className="space-y-3">
            <h2 className="text-[13px] font-medium uppercase tracking-wider text-gray-8">Users</h2>

            {!isReady || isLoading ? (
              <Card className="p-6">
                <div className="flex items-center gap-3 text-[13px] text-gray-8">
                  <div className="w-4 h-4 border-2 border-gray-7 border-t-gray-12 rounded-full animate-spin" />
                  <span>Loading users…</span>
                </div>
              </Card>
            ) : users.length === 0 ? (
              <Card className="p-6">
                <p className="text-[13px] text-gray-8">No users found.</p>
              </Card>
            ) : (
              <Card className="overflow-hidden">
                <div className="overflow-x-auto">
                  <table className="min-w-full text-left">
                    <thead className="border-b border-gray-3 bg-gray-1/60">
                      <tr>
                        <th className="px-4 py-2.5 text-[11px] uppercase tracking-wider text-gray-7 font-medium">Name</th>
                        <th className="px-4 py-2.5 text-[11px] uppercase tracking-wider text-gray-7 font-medium">Email</th>
                        <th className="px-4 py-2.5 text-[11px] uppercase tracking-wider text-gray-7 font-medium">Karna user ID</th>
                        <th className="px-4 py-2.5 text-right text-[11px] uppercase tracking-wider text-gray-7 font-medium">Copy</th>
                      </tr>
                    </thead>
                    <tbody>
                      {users.map((user) => (
                        <tr key={user.id} className="border-b border-gray-3 last:border-b-0">
                          <td className="px-4 py-3 text-[13px] text-gray-11">
                            <div className="inline-flex items-center gap-2">
                              <span>{user.name?.trim() || 'Unnamed user'}</span>
                              {isCurrentUser(user) && <Badge tone="accent">You</Badge>}
                            </div>
                          </td>
                          <td className="px-4 py-3 text-[13px] text-gray-8">{user.email?.trim() || 'No email'}</td>
                          <td className="px-4 py-3">
                            <span className="font-mono text-[12px] text-gray-11 break-all">{user.id}</span>
                          </td>
                          <td className="px-4 py-3 text-right">
                            <CopyValueButton value={user.id} label={`ID for ${user.name?.trim() || user.email?.trim() || user.id}`} />
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </Card>
            )}
          </section>

          <Card className="p-4">
            <div className="flex items-start gap-2.5">
              <Info size={16} weight="bold" className="text-sun-10 mt-0.5 flex-shrink-0" />
              <p className="text-[13px] leading-[1.5] text-gray-8">
                Slack <code className="font-mono text-gray-10">user_map</code> entries use the format{' '}
                <code className="font-mono text-gray-10">Slack user ID -&gt; Karna user ID</code>. Get Slack user IDs in Slack (for example, open a member profile and use
                {' '}<span className="text-gray-10">Copy member ID</span>), then map each one to the Karna ID shown above.
              </p>
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
}
