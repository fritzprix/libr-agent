import { Moon, Sun } from 'lucide-react';
import { useTheme } from 'next-themes';
import { Button } from '../ui';

export function ThemeToggle() {
  const { theme, setTheme } = useTheme();

  const toggleTheme = () => {
    setTheme(theme === 'dark' ? 'light' : 'dark');
  };
  return (
    <Button variant="ghost" size="sm" onClick={toggleTheme}>
      {theme === 'dark' ? <Sun size={16} /> : <Moon size={16} />}
    </Button>
  );
}
