'use client';

import { forwardRef } from 'react';
import { cn } from '@/lib/utils';

export const inputBaseClass =
  'w-full rounded-lg bg-gray-2 border border-gray-4 text-gray-12 placeholder:text-gray-7 ' +
  'transition-smooth focus-ring focus-visible:border-gray-6 disabled:opacity-50';

export const Input = forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>(
  ({ className, ...props }, ref) => (
    <input
      ref={ref}
      className={cn(inputBaseClass, 'h-9 px-3 text-[14px]', className)}
      {...props}
    />
  ),
);
Input.displayName = 'Input';
