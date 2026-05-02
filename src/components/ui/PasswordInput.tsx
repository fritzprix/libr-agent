import * as React from 'react';
import { Input } from './input';
import { Button } from './button';
import { Eye, EyeOff } from 'lucide-react';
import { cn } from '@/lib/utils';
import { useTranslation } from 'react-i18next';

export interface PasswordInputProps
  extends React.InputHTMLAttributes<HTMLInputElement> {
  wrapperClassName?: string;
}

const PasswordInput = React.forwardRef<HTMLInputElement, PasswordInputProps>(
  ({ className, wrapperClassName, ...props }, ref) => {
    const [showPassword, setShowPassword] = React.useState(false);
    const { t } = useTranslation('common');

    return (
      <div className={cn('relative', wrapperClassName)}>
        <Input
          type={showPassword ? 'text' : 'password'}
          className={cn('pr-10', className)}
          ref={ref}
          {...props}
        />
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="absolute right-0 top-0 h-full px-3 py-2 hover:bg-transparent"
          onClick={() => setShowPassword((prev) => !prev)}
          aria-label={
            showPassword
              ? t('components.passwordInput.hide', 'Hide value')
              : t('components.passwordInput.show', 'Show value')
          }
          aria-pressed={showPassword}
        >
          {showPassword ? (
            <EyeOff className="h-4 w-4 text-muted-foreground" />
          ) : (
            <Eye className="h-4 w-4 text-muted-foreground" />
          )}
        </Button>
      </div>
    );
  },
);
PasswordInput.displayName = 'PasswordInput';

export { PasswordInput };
