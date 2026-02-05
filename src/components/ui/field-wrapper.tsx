import React from 'react';
import { cn } from '@/lib/utils';
import { Label } from '@/components/ui/label';

export interface FieldWrapperProps {
  label?: string;
  error?: string;
  inputId?: string;
  errorId?: string;
  containerClassName?: string;
  labelClassName?: string;
  children: React.ReactNode;
}

export function FieldWrapper({
  label,
  error,
  inputId,
  errorId,
  containerClassName,
  labelClassName,
  children,
}: FieldWrapperProps) {
  return (
    <div className={containerClassName}>
      {label && (
        <Label
          htmlFor={inputId}
          className={cn('block mb-2 font-medium', labelClassName)}
        >
          {label}
        </Label>
      )}
      {children}
      {error && (
        <p id={errorId} className="text-destructive text-xs mt-1">
          {error}
        </p>
      )}
    </div>
  );
}
