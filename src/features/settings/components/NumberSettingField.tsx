import type { ReactNode } from 'react';
import { Input } from '@/components/ui';

interface NumberSettingFieldProps {
  label: string;
  description: string;
  value: number;
  onValueChange: (value: number) => void;
  parseValue: (rawValue: string) => number;
  placeholder?: string;
  min?: number;
  max?: number;
  step?: number;
  labelAdornment?: ReactNode;
  containerClassName?: string;
  inputClassName?: string;
}

export function NumberSettingField({
  label,
  description,
  value,
  onValueChange,
  parseValue,
  placeholder,
  min,
  max,
  step,
  labelAdornment,
  containerClassName = 'min-w-0 rounded-xl border border-border/70 p-4',
  inputClassName = 'bg-background border text-foreground w-full max-w-xs',
}: NumberSettingFieldProps) {
  return (
    <div className={containerClassName}>
      <div className="mb-2 flex items-center gap-2">
        <label className="block text-muted-foreground font-medium">
          {label}
        </label>
        {labelAdornment}
      </div>
      <Input
        type="number"
        placeholder={placeholder}
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(event) => {
          onValueChange(parseValue(event.target.value));
        }}
        className={inputClassName}
      />
      <p className="mt-1 text-xs text-muted-foreground">{description}</p>
    </div>
  );
}
