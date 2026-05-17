'use client';

import { useSession } from 'next-auth/react';
import Link from 'next/link';
import { useAuthDisabled } from '@/lib/auth-context';
import { useAgents } from '@/hooks/use-tasks';
import { Robot, Lightning } from '@phosphor-icons/react';

export default function AgentsIndexPage() {
  const authDisabled = useAuthDisabled();
  const { status: authStatus } = useSession();
  const isReady = authDisabled || authStatus === 'authenticated';
  const { data: agents = [], isLoading } = useAgents(isReady);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="w-5 h-5 border-2 border-gray-7 border-t-gray-12 rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-3xl mx-auto px-4 sm:px-6 py-6">
        <div className="mb-6">
          <h2 className="text-[18px] font-semibold text-gray-12 tracking-[-0.02em]">Agents</h2>
          <p className="text-[13px] text-gray-8 mt-0.5">
            Named agent identities seeded from <code className="font-mono text-gray-9">config.yaml</code>. Pause one to stop pickup of its tasks without affecting others.
          </p>
        </div>

        {agents.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-20 text-gray-8">
            <Robot size={48} weight="thin" className="mb-4" />
            <p className="text-[15px] font-medium text-gray-10">No agent profiles yet</p>
            <p className="text-[13px] mt-1.5 max-w-md text-center">
              The agent worker seeds profiles from your config.yaml backends on startup.
              Restart the agent if you've just added a backend.
            </p>
          </div>
        ) : (
          <div className="space-y-2">
            {agents.map((a) => (
              <Link
                key={a.id}
                href={`/agents/${a.id}`}
                className="block bg-gray-2 border border-gray-3 rounded-lg px-4 py-3 hover:bg-gray-3 transition-colors"
              >
                <div className="flex items-center gap-3">
                  <span className="text-[24px]" aria-hidden>{a.avatar_emoji}</span>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-[14px] font-medium text-gray-12">{a.name}</span>
                      {a.is_default && (
                        <span className="inline-flex items-center gap-1 px-1.5 h-4 rounded text-[10px] bg-sun-9/15 border border-sun-9/30 text-sun-10">
                          <Lightning size={10} weight="bold" /> default
                        </span>
                      )}
                      {a.paused_reason && (
                        <span className="inline-flex items-center px-1.5 h-4 rounded text-[10px] bg-amber-500/15 border border-amber-500/30 text-amber-400">
                          paused
                        </span>
                      )}
                    </div>
                    <div className="text-[11px] text-gray-7 font-mono mt-0.5">
                      {a.cli} · {a.model}
                    </div>
                  </div>
                </div>
              </Link>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
