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
  // value가 options에 있는지 확인하고, 없으면 빈 문자열 사용
  const validValue = options.some((opt) => opt.value === value) ? value : '';

  return (
    <div className={`relative ${className}`}>
      <Select
        key={options.length} // options 변경시 강제 리렌더링
        value={validValue}
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
