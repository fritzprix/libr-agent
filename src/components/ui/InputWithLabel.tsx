import React, { useId } from 'react';
import { Input } from './input';
import { FieldWrapper } from './field-wrapper';
import { cn } from '@/lib/utils';

interface InputWithLabelProps
  extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
  containerClassName?: string;
  variant?: 'default' | 'terminal';
}

export default function InputWithLabel({
  label,
  error,
  className,
  containerClassName,
  variant = 'default',
  ...props
}: InputWithLabelProps) {
  const generatedId = useId();
  const inputId = props.id || generatedId;
  const errorId = error ? `${inputId}-error` : undefined;

  // For terminal variant, render input directly without wrapper
  if (variant === 'terminal') {
    return (
      <Input
        id={inputId}
        aria-describedby={errorId}
        aria-invalid={!!error}
        className={cn(
          'w-full bg-transparent border-none outline-none text-success px-0 py-1 terminal-input focus:ring-2 focus:ring-success focus:ring-offset-2 focus:ring-offset-black transition-all duration-200',
          className,
        )}
        {...props}
      />
    );
  }

  // For default variant, use wrapper div
  return (
    <FieldWrapper
      label={label}
      error={error}
      inputId={inputId}
      errorId={errorId}
      containerClassName={containerClassName}
      labelClassName="text-muted-foreground"
    >
      <Input
        id={inputId}
        aria-describedby={errorId}
        aria-invalid={!!error}
        className={cn(error && 'border-destructive', className)}
        {...props}
      />
    </FieldWrapper>
  );
}
