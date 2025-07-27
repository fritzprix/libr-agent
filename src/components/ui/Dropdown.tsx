import { SelectTrigger } from '@radix-ui/react-select';
import { Select, SelectContent, SelectItem, SelectValue } from './select';

interface DropdownOption {
  value: string;
  label: string;
  disabled?: boolean;
}

interface DropdownProps {
  options: DropdownOption[];
  value?: string;
  onChange: (value: string) => void;
  placeholder: string;
  disabled?: boolean;
  className?: string;
  variant?: 'default' | 'compact';
}

export function Dropdown({
  options,
  value,
  onChange,
  placeholder,
  disabled,
  className = '',
}: DropdownProps) {
  return (
    <div className={`relative ${className}`}>
      <Select
        value={typeof value === 'string' ? value : `${value}`}
        onValueChange={(v) => onChange(v)}
        disabled={disabled}
      >
        <SelectTrigger>
          <SelectValue placeholder={placeholder} />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem
              key={option.value}
              value={option.value}
              disabled={option.disabled}
            >
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}
