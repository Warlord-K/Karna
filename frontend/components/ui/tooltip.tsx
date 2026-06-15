'use client';

import { useId, useRef, useState } from 'react';
import { cn } from '@/lib/utils';

type Side = 'top' | 'bottom' | 'left' | 'right';

interface TooltipProps {
  label: React.ReactNode;
  side?: Side;
  delay?: number;
  children: React.ReactElement;
  className?: string;
}

const SIDE_CLASSES: Record<Side, string> = {
  top: 'bottom-full left-1/2 -translate-x-1/2 mb-1.5',
  bottom: 'top-full left-1/2 -translate-x-1/2 mt-1.5',
  left: 'right-full top-1/2 -translate-y-1/2 mr-1.5',
  right: 'left-full top-1/2 -translate-y-1/2 ml-1.5',
};

/**
 * Minimal, dependency-free tooltip. Shows on hover + keyboard focus, respects a
 * short open delay, and renders an accessible description. For complex anchored
 * menus prefer a real popover; this is for short hint labels.
 */
export function Tooltip({ label, side = 'top', delay = 250, children, className }: TooltipProps) {
  const [open, setOpen] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const id = useId();

  const show = () => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => setOpen(true), delay);
  };
  const hide = () => {
    if (timer.current) clearTimeout(timer.current);
    setOpen(false);
  };

  return (
    <span
      className="relative inline-flex"
      onMouseEnter={show}
      onMouseLeave={hide}
      onFocusCapture={show}
      onBlurCapture={hide}
      aria-describedby={open ? id : undefined}
    >
      {children}
      {open && (
        <span
          role="tooltip"
          id={id}
          className={cn(
            'pointer-events-none absolute z-50 whitespace-nowrap rounded-md border border-gray-5 bg-gray-3 px-2 py-1',
            'text-[11px] font-medium text-gray-11 shadow-elevated animate-fade-in',
            SIDE_CLASSES[side],
            className,
          )}
        >
          {label}
        </span>
      )}
    </span>
  );
}
