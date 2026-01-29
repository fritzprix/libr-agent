import React from 'react';
import { cn } from '@/lib/utils';
import { Label } from '@/components/ui/label';

export interface FieldWrapperProps {
  label?: string;
  error?: string;
  containerClassName?: string;
  labelClassName?: string;
  children: React.ReactNode;
}

export function FieldWrapper({
  label,
  error,
  containerClassName,
  labelClassName,
  children,
}: FieldWrapperProps) {
  return (
    <div className={containerClassName}>
      {label && (
        <Label className={cn('block mb-2 font-medium', labelClassName)}>
          {label}
        </Label>
      )}
      {children}
      {error && <p className="text-destructive text-xs mt-1">{error}</p>}
    </div>
  );
}
