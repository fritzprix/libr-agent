import React from 'react';
import { Input } from './input';

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
  className = '',
  containerClassName = '',
  variant = 'default',
  ...props
}: InputWithLabelProps) {
  // For terminal variant, render input directly without wrapper
  if (variant === 'terminal') {
    return (
      <Input
        className={`w-full bg-transparent border-none outline-none text-green-400 px-0 py-1 terminal-input focus:ring-2 focus:ring-green-400 focus:ring-offset-2 focus:ring-offset-black transition-all duration-200 ${className}`}
        {...props}
      />
    );
  }

  // For default variant, use wrapper div
  return (
    <div className={containerClassName}>
      {label && (
        <label className="block text-gray-400 mb-2 font-medium">{label}</label>
      )}
      <Input
        className={`${error ? 'border-red-400' : ''} ${className}`}
        {...props}
      />
      {error && <p className="text-red-400 text-xs mt-1">{error}</p>}
    </div>
  );
}
