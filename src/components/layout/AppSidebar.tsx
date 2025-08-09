import {
  BrainCircuit,
  History,
  MessageSquare,
  Settings,
  Users,
} from 'lucide-react';
import React, { useEffect, useMemo } from 'react';
import { Link, useLocation } from 'react-router-dom';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  useSidebar,
} from '../ui/sidebar';
import { useSessionContext } from '@/context/SessionContext';
import SessionList from '@/features/session/SessionList';

interface AppSidebarProps {
  onOpenSettings: () => void;
}

export default function AppSidebar({ onOpenSettings }: AppSidebarProps) {
  const { state, toggleSidebar } = useSidebar();
  const { select, sessions: sessionPages } = useSessionContext();
  const location = useLocation();

  const sessions = useMemo(
    () => (sessionPages ? sessionPages.flatMap((p) => p.items) : []),
    [sessionPages],
  );

  const isCollapsed = state === 'collapsed';
  const currentView = location.pathname.substring(1); // Extract current view from path

  // Add keyboard shortcut for sidebar toggle
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.ctrlKey && event.key === 'b') {
        event.preventDefault();
        toggleSidebar();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [toggleSidebar]);

  return (
    <Sidebar
      className="backdrop-blur-sm border-r shadow-xl"
      collapsible="icon"
      style={
        {
          '--sidebar-width-icon': '3.5rem',
        } as React.CSSProperties
      }
    >
      <SidebarHeader className="border-b">
        <div className="flex flex-row items-center justify-center gap-2 p-4">
          <BrainCircuit size={32} className="flex-shrink-0" />
          <span
            className={`font-medium text-2xl whitespace-nowrap transition-all duration-300 ease-in-out ${
              isCollapsed
                ? 'opacity-0 w-0 overflow-hidden'
                : 'opacity-100 w-auto'
            }`}
          >
            SynapticFlow
          </span>
        </div>
      </SidebarHeader>

      <SidebarContent className={`flex-1 overflow-y-auto  terminal-scrollbar`}>
        {/* Chat Section */}
        <SidebarGroup>
          <SidebarGroupLabel className="text-sm font-semibold uppercase tracking-wide mb-2">
            Chat
          </SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <Link to="/chat/single">
                  <SidebarMenuButton
                    onClick={() => select()}
                    isActive={location.pathname === '/chat/single'}
                    tooltip="Single Assistant Chat"
                  >
                    <MessageSquare size={16} />
                    <span>Single Assistant Chat</span>
                  </SidebarMenuButton>
                </Link>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <Link to="/chat/group">
                  <SidebarMenuButton
                    onClick={() => select()}
                    isActive={location.pathname === '/chat/group'}
                    tooltip="Assistant Group Chat"
                  >
                    <Users size={16} />
                    <span>Assistant Group Chat</span>
                  </SidebarMenuButton>
                </Link>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <Link to="/chat/flow">
                  <SidebarMenuButton
                    onClick={() => select()}
                    isActive={location.pathname === '/chat/flow'}
                    tooltip="Flow Chat"
                  >
                    <BrainCircuit size={16} />
                    <span>Flow Chat</span>
                  </SidebarMenuButton>
                </Link>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        {/* Group Section */}
        <SidebarGroup>
          <SidebarGroupLabel className="text-sm font-semibold uppercase tracking-wide mb-2">
            Assistant Management
          </SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <Link to="/assistants">
                  <SidebarMenuButton
                    isActive={location.pathname === '/assistants'}
                    tooltip="Manage Assistants"
                  >
                    <Users size={16} />
                    <span>Assistants</span>
                  </SidebarMenuButton>
                </Link>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <Link to="/assistants/groups">
                  <SidebarMenuButton
                    isActive={location.pathname === '/assistants/groups'}
                    tooltip="Manage Assistant Groups"
                  >
                    <Users size={16} />
                    <span>Assistant Groups</span>
                  </SidebarMenuButton>
                </Link>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        {/* History Section */}
        <SidebarGroup>
          <SidebarGroupLabel className="text-sm font-semibold uppercase tracking-wide mb-2">
            History
          </SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <Link to="/history/search">
                  <SidebarMenuButton
                    isActive={location.pathname === '/history/search'}
                    tooltip="Search History"
                  >
                    <History size={16} />
                    <span>Search History</span>
                  </SidebarMenuButton>
                </Link>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        {/* Recent Sessions - only show if not in history view */}
        {currentView !== 'history' && sessions.length > 0 && (
          <SidebarGroup>
            {!isCollapsed && (
              <SidebarGroupLabel className="text-xs font-semibold uppercase tracking-wider mb-2">
                Recent Sessions
              </SidebarGroupLabel>
            )}
            <SidebarGroupContent>
              <SessionList
                sessions={sessions.slice(0, 5)}
                showSearch={false}
                emptyMessage=""
              />
            </SidebarGroupContent>
          </SidebarGroup>
        )}
      </SidebarContent>
 <SidebarGroup>
          <SidebarGroupLabel className="text-sm font-semibold uppercase tracking-wide mb-2">
            History
          </SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <Link to="/tools/webmcp-demo">
                  <SidebarMenuButton
                    isActive={location.pathname === '/tools/webmcp-demo'}
                    tooltip="Demo"
                  >
                    <History size={16} />
                    <span>Demo</span>
                  </SidebarMenuButton>
                </Link>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      <SidebarFooter className="border-t">
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              onClick={onOpenSettings}
              tooltip="Settings"
              className={`transition-all duration-200`}
            >
              <Settings size={16} />
              {!isCollapsed && <span>Settings</span>}
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  );
}
