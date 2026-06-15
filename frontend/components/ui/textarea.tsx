'use client';

import { forwardRef } from 'react';
import { cn } from '@/lib/utils';
import { inputBaseClass } from './input';

export const Textarea = forwardRef<HTMLTextAreaElement, React.TextareaHTMLAttributes<HTMLTextAreaElement>>(
  ({ className, ...props }, ref) => (
    <textarea
      ref={ref}
      className={cn(inputBaseClass, 'px-3 py-2 text-[14px] leading-[1.5] resize-none', className)}
      {...props}
    />
  ),
);
Textarea.displayName = 'Textarea';
