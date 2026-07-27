import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Input } from '@/components/ui';
import { isIntegerInRange } from '@/features/settings/components/settings-number-utils';

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
  const [draft, setDraft] = useState(() => String(value));
  const isFocusedRef = useRef(false);

  useEffect(() => {
    if (!isFocusedRef.current) {
      setDraft(String(value));
    }
  }, [value]);

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
        value={draft}
        onFocus={() => {
          isFocusedRef.current = true;
        }}
        onChange={(event) => {
          const rawValue = event.target.value;
          setDraft(rawValue);

          // Keep incomplete / out-of-range drafts local so typing "256" with
          // min=32 is not rewritten to "32" on the first digit.
          if (!isIntegerInRange(rawValue, { min, max })) {
            return;
          }

          onValueChange(parseValue(rawValue));
        }}
        onBlur={() => {
          const committed = parseValue(draft);
          isFocusedRef.current = false;
          setDraft(String(committed));
          onValueChange(committed);
        }}
        className={inputClassName}
      />
      <p className="mt-1 text-xs text-muted-foreground">{description}</p>
    </div>
  );
}
