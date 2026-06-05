import { useTheme } from 'next-themes';

/**
 * Custom hook to detect if dark mode is active from the centralized theme state.
 * Uses next-themes resolvedTheme to ensure consistency with the application theme.
 */
export function useIsDarkMode() {
  const { resolvedTheme } = useTheme();
  return resolvedTheme === 'dark';
}
