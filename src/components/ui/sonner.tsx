import { useIsDarkMode } from '@/hooks/use-is-dark-mode';
import React from 'react';
import { useTranslation } from 'react-i18next';
import { Toaster as Sonner, ToasterProps } from 'sonner';

const Toaster = ({
  closeButton = true,
  toastOptions,
  ...props
}: ToasterProps) => {
  const isDark = useIsDarkMode();
  const { t } = useTranslation();

  return (
    <Sonner
      closeButton={closeButton}
      theme={isDark ? 'dark' : 'light'}
      className="toaster group"
      toastOptions={{
        closeButtonAriaLabel: t('common.close', 'Close'),
        ...toastOptions,
      }}
      style={
        {
          '--normal-bg': 'var(--popover)',
          '--normal-text': 'var(--popover-foreground)',
          '--normal-border': 'var(--border)',
        } as React.CSSProperties
      }
      {...props}
    />
  );
};

export { Toaster };
