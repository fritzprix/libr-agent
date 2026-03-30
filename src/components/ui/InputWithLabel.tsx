import React, { useId } from 'react';
import { Input } from './input';
import { FieldWrapper } from './field-wrapper';
import { cn } from '@/lib/utils';

interface InputWithLabelProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
  containerClassName?: string;
}

export default function InputWithLabel({
  label,
  error,
  className,
  containerClassName,
  ...props
}: InputWithLabelProps) {
  const generatedId = useId();
  const inputId = props.id || generatedId;
  const errorId = error ? `${inputId}-error` : undefined;

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
