import React, { useId } from 'react';
import { Textarea } from './textarea';
import { FieldWrapper } from './field-wrapper';
import { cn } from '@/lib/utils';

interface TextareaWithLabelProps
  extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  label?: string;
  error?: string;
  containerClassName?: string;
}

export default function TextareaWithLabel({
  label,
  error,
  className,
  containerClassName,
  ...props
}: TextareaWithLabelProps) {
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
      <Textarea
        id={inputId}
        aria-describedby={errorId}
        aria-invalid={!!error}
        className={cn(error && 'border-destructive', className)}
        {...props}
      />
    </FieldWrapper>
  );
}
