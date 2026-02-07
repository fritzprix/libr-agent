import { useTheme } from 'next-themes';

/**
 * Custom hook to detect dark mode from centralized theme state
 * Uses next-themes resolvedTheme to ensure consistency with app theme
 */
export function useIsDarkMode() {
  const { resolvedTheme } = useTheme();
  return resolvedTheme === 'dark';
}
