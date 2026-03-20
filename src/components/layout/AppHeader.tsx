import { ReactNode } from 'react';
import { SidebarTrigger } from '../ui/sidebar';

export function AppHeader({ children }: { children?: ReactNode }) {
  return (
    <header className="flex h-16 items-center px-4 border-b flex-shrink-0 justify-between bg-background">
      <SidebarTrigger />
      <div className="border rounded-lg p-1.5">{children}</div>
    </header>
  );
}
