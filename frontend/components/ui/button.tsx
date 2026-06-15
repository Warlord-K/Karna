'use client';

import { forwardRef } from 'react';
import { Slot } from '@radix-ui/react-slot';
import { cn } from '@/lib/utils';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'outline' | 'danger';
export type ButtonSize = 'sm' | 'md' | 'lg' | 'icon' | 'icon-sm';

const VARIANTS: Record<ButtonVariant, string> = {
  primary:
    'bg-sun-9 text-gray-1 hover:bg-sun-10 hover:shadow-[0_0_16px_hsl(40_90%_56%/0.22)] disabled:hover:shadow-none',
  secondary:
    'bg-gray-3 text-gray-12 border border-gray-4 hover:bg-gray-4 hover:border-gray-5',
  ghost:
    'text-gray-9 hover:text-gray-12 hover:bg-gray-3',
  outline:
    'border border-gray-4 text-gray-11 hover:bg-gray-3 hover:border-gray-5',
  danger:
    'bg-red-500/90 text-white hover:bg-red-500',
};

const SIZES: Record<ButtonSize, string> = {
  sm: 'h-7 px-2.5 text-[12px] gap-1.5 rounded-md',
  md: 'h-8 px-3 text-[13px] gap-1.5 rounded-lg',
  lg: 'h-9 px-4 text-[14px] gap-2 rounded-lg',
  icon: 'h-8 w-8 rounded-lg',
  'icon-sm': 'h-7 w-7 rounded-md',
};

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  asChild?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant = 'secondary', size = 'md', asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : 'button';
    return (
      <Comp
        ref={ref}
        className={cn(
          'inline-flex items-center justify-center font-medium tracking-[-0.01em] whitespace-nowrap select-none',
          'transition-smooth press-scale focus-ring',
          'disabled:opacity-50 disabled:pointer-events-none',
          VARIANTS[variant],
          SIZES[size],
          className,
        )}
        {...props}
      />
    );
  },
);
Button.displayName = 'Button';
