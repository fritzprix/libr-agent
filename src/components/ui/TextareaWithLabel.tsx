import React from 'react';
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
  return (
    <FieldWrapper
      label={label}
      error={error}
      containerClassName={containerClassName}
      labelClassName="text-muted-foreground"
    >
      <Textarea
        className={cn(error && 'border-red-400', className)}
        {...props}
      />
    </FieldWrapper>
  );
}
