'use client';

import { forwardRef } from 'react';
import { cn } from '@/lib/utils';

export type BadgeTone =
  | 'neutral'
  | 'accent'
  | 'success'
  | 'warning'
  | 'danger'
  | 'info'
  | 'purple';

const TONES: Record<BadgeTone, string> = {
  neutral: 'bg-gray-3 border-gray-5 text-gray-9',
  accent: 'bg-sun-9/15 border-sun-9/30 text-sun-10',
  success: 'bg-green-500/15 border-green-500/30 text-green-400',
  warning: 'bg-amber-500/15 border-amber-500/30 text-amber-400',
  danger: 'bg-red-500/15 border-red-500/30 text-red-400',
  info: 'bg-blue-500/15 border-blue-500/30 text-blue-400',
  purple: 'bg-purple-500/15 border-purple-500/30 text-purple-400',
};

export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  tone?: BadgeTone;
}

export const Badge = forwardRef<HTMLSpanElement, BadgeProps>(
  ({ className, tone = 'neutral', ...props }, ref) => (
    <span
      ref={ref}
      className={cn(
        'inline-flex items-center gap-1 px-1.5 h-[18px] rounded border text-[10px] font-medium tracking-[-0.01em] leading-none',
        TONES[tone],
        className,
      )}
      {...props}
    />
  ),
);
Badge.displayName = 'Badge';
