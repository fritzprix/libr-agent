import React from 'react';
import { Button } from './button';
import { cn } from '@/lib/utils';

interface ButtonLegacyProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?:
    | 'primary'
    | 'secondary'
    | 'danger'
    | 'ghost'
    | 'default'
    | 'destructive'
    | 'outline'
    | 'link';
  size?: 'sm' | 'md' | 'lg' | 'default' | 'icon';
  children: React.ReactNode;
}

export default function ButtonLegacy({
  variant = 'secondary',
  size = 'md',
  className = '',
  children,
  ...props
}: ButtonLegacyProps) {
  // Map legacy variants to Shadcn variants
  const mapVariant = (v: string) => {
    switch (v) {
      case 'primary':
        return 'default';
      case 'secondary':
        return 'secondary';
      case 'danger':
        return 'destructive';
      case 'ghost':
        return 'ghost';
      default:
        return v as
          | 'default'
          | 'destructive'
          | 'outline'
          | 'secondary'
          | 'ghost'
          | 'link';
    }
  };

  // Map legacy sizes to Shadcn sizes
  const mapSize = (s: string) => {
    switch (s) {
      case 'sm':
        return 'sm';
      case 'md':
        return 'default';
      case 'lg':
        return 'lg';
      default:
        return s as 'default' | 'sm' | 'lg' | 'icon';
    }
  };

  // Apply terminal-style colors for certain variants if needed
  const terminalStyles = {
    primary:
      'bg-green-900/20 border-green-400 text-green-400 hover:bg-green-900/40',
    danger: 'bg-red-900/20 border-red-400 text-red-400 hover:bg-red-900/40',
  };

  const additionalClassName =
    variant === 'primary' || variant === 'danger'
      ? cn('border', terminalStyles[variant as keyof typeof terminalStyles])
      : '';

  return (
    <Button
      variant={mapVariant(variant)}
      size={mapSize(size)}
      className={cn(additionalClassName, className)}
      {...props}
    >
      {children}
    </Button>
  );
}
