import React, { useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Check } from 'lucide-react';
import { Label } from '@/components/ui';
import { cn } from '@/lib/utils';
import { type ColorTheme } from '@/lib/services/settings-service';

export interface ColorThemePickerProps {
  value?: ColorTheme;
  onChange: (theme: ColorTheme) => void;
  className?: string;
}

interface ThemeConfig {
  id: ColorTheme;
  labelKey: string;
  fallback: string;
  color: string;
}

const THEME_CONFIGS: readonly ThemeConfig[] = [
  {
    id: 'neutral',
    labelKey: 'settings.display.colorThemes.neutral',
    fallback: 'Neutral (Monochrome)',
    color: 'oklch(0.55 0 0)',
  },
  {
    id: 'amber',
    labelKey: 'settings.display.colorThemes.amber',
    fallback: 'Libr Amber (Warmth)',
    color: 'oklch(0.530 0.165 58)',
  },
  {
    id: 'violet',
    labelKey: 'settings.display.colorThemes.violet',
    fallback: 'Iris Violet (AI)',
    color: 'oklch(0.520 0.220 285)',
  },
  {
    id: 'ocean',
    labelKey: 'settings.display.colorThemes.ocean',
    fallback: 'Ocean Blue (Calm)',
    color: 'oklch(0.500 0.170 245)',
  },
  {
    id: 'forest',
    labelKey: 'settings.display.colorThemes.forest',
    fallback: 'Forest Sage (Focus)',
    color: 'oklch(0.480 0.150 150)',
  },
];

export function ColorThemePicker({
  value = 'neutral',
  onChange,
  className,
}: ColorThemePickerProps) {
  const { t } = useTranslation('common');
  const buttonRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const handleKeyDown = (e: React.KeyboardEvent, index: number) => {
    let nextIndex: number | null = null;
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      e.preventDefault();
      nextIndex = (index + 1) % THEME_CONFIGS.length;
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      e.preventDefault();
      nextIndex = (index - 1 + THEME_CONFIGS.length) % THEME_CONFIGS.length;
    } else if (e.key === 'Home') {
      e.preventDefault();
      nextIndex = 0;
    } else if (e.key === 'End') {
      e.preventDefault();
      nextIndex = THEME_CONFIGS.length - 1;
    }

    if (nextIndex !== null) {
      const nextTheme = THEME_CONFIGS[nextIndex];
      onChange(nextTheme.id);
      buttonRefs.current[nextIndex]?.focus();
    }
  };

  return (
    <div className={cn('min-w-0 space-y-2', className)}>
      <div>
        <Label
          id="color-theme-label"
          className="mb-1 block text-muted-foreground"
        >
          {t('settings.display.colorTheme', 'Color Theme')}
        </Label>
        <p className="text-xs text-muted-foreground">
          {t(
            'settings.display.colorThemeDescription',
            'Choose an accent color theme for the interface.',
          )}
        </p>
      </div>

      <div
        role="radiogroup"
        aria-labelledby="color-theme-label"
        className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-5 gap-2.5 pt-1"
      >
        {THEME_CONFIGS.map((theme, index) => {
          const isSelected = value === theme.id;
          return (
            <button
              key={theme.id}
              ref={(el) => {
                buttonRefs.current[index] = el;
              }}
              type="button"
              role="radio"
              aria-checked={isSelected}
              tabIndex={isSelected ? 0 : -1}
              onClick={() => onChange(theme.id)}
              onKeyDown={(e) => handleKeyDown(e, index)}
              className={cn(
                'flex items-center gap-2.5 px-3 py-2.5 rounded-lg border text-sm font-medium transition-all text-left focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-1',
                isSelected
                  ? 'border-primary bg-primary/10 text-foreground ring-1 ring-primary'
                  : 'border-border/80 hover:border-border hover:bg-muted/50 text-muted-foreground hover:text-foreground',
              )}
            >
              <span
                className="w-3.5 h-3.5 rounded-full shrink-0 shadow-xs border border-black/10 dark:border-white/20"
                style={{ backgroundColor: theme.color }}
                aria-hidden="true"
              />
              <span className="truncate flex-1 text-xs font-medium">
                {t(theme.labelKey, theme.fallback)}
              </span>
              {isSelected && (
                <Check className="w-3.5 h-3.5 shrink-0 text-primary" />
              )}
            </button>
          );
        })}
      </div>
    </div>
  );
}

export default ColorThemePicker;
