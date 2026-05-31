import { useTheme } from 'next-themes';
import React from 'react';
import { Toaster as Sonner, ToasterProps } from 'sonner';

const Toaster = ({
  closeButton = true,
  toastOptions,
  ...props
}: ToasterProps) => {
  // Prefer resolvedTheme so the toaster receives the actual theme being used
  const { resolvedTheme } = useTheme();

  return (
    <Sonner
      closeButton={closeButton}
      theme={(resolvedTheme ?? 'system') as ToasterProps['theme']}
      className="toaster group"
      toastOptions={{
        closeButtonAriaLabel: 'Dismiss notification',
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
