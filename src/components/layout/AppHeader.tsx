import { ReactNode } from 'react';
import { SidebarTrigger } from '../ui/sidebar';
import { TooltipProvider } from '../ui/tooltip';

export function AppHeader({ children }: { children?: ReactNode }) {
  return (
    <header className="flex h-16 items-center px-4 border-b flex-shrink-0 justify-between bg-background">
      <SidebarTrigger />
      <TooltipProvider delayDuration={300}>
        <div className="border rounded-lg p-1.5 flex items-center gap-1">
          {children}
        </div>
      </TooltipProvider>
    </header>
  );
}
