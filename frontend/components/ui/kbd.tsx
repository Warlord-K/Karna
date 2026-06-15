'use client';

import { cn } from '@/lib/utils';

export function Kbd({ className, children, ...props }: React.HTMLAttributes<HTMLElement>) {
  return (
    <kbd
      className={cn(
        'inline-flex items-center justify-center min-w-[18px] h-[18px] px-1',
        'rounded border border-gray-5 bg-gray-3 text-gray-9',
        'text-[10px] font-medium font-sans leading-none',
        className,
      )}
      {...props}
    >
      {children}
    </kbd>
  );
}
