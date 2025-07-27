import { ReactNode } from 'react';
import { SidebarTrigger } from './ui/sidebar';

export function AppHeader({ children }: { children?: ReactNode }) {
  return (
    <header className="flex items-center p-4 border-b flex-shrink-0 justify-between">
      <SidebarTrigger />
      <div className="border rounded-2xl p-2">{children}</div>
    </header>
  );
}
