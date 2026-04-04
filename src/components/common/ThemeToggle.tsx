import { Moon, Sun } from 'lucide-react';
import { useTheme } from 'next-themes';
import { useTranslation } from 'react-i18next';
import { Button } from '../ui';
import { Tooltip, TooltipContent, TooltipTrigger } from '../ui/tooltip';

export function ThemeToggle() {
  // Use resolvedTheme to get the concrete theme value (considers system when 'system' is selected)
  const { resolvedTheme, setTheme } = useTheme();
  const { t } = useTranslation();

  const toggleTheme = () => {
    setTheme(resolvedTheme === 'dark' ? 'light' : 'dark');
  };

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="sm"
          onClick={toggleTheme}
          aria-label={t('theme.toggle')}
        >
          {resolvedTheme === 'dark' ? <Sun size={16} /> : <Moon size={16} />}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{t('theme.toggle')}</TooltipContent>
    </Tooltip>
  );
}
