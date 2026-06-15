'use client';

import { cn } from '@/lib/utils';

interface PageHeaderProps {
  title: string;
  description?: React.ReactNode;
  actions?: React.ReactNode;
  className?: string;
}

export function PageHeader({ title, description, actions, className }: PageHeaderProps) {
  return (
    <div className={cn('flex items-start justify-between gap-3 mb-6', className)}>
      <div className="min-w-0">
        <h1 className="text-[18px] font-semibold text-gray-12 tracking-[-0.02em]">{title}</h1>
        {description && (
          <p className="text-[13px] text-gray-8 mt-0.5 leading-[1.5]">{description}</p>
        )}
      </div>
      {actions && <div className="flex items-center gap-2 flex-shrink-0">{actions}</div>}
    </div>
  );
}
