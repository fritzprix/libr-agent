import { Bot, BrainCircuit, History, Settings, Users } from 'lucide-react';
import React from 'react';
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
import { useNavigate } from 'react-router-dom';
// Remove modal import; we'll navigate to a dedicated settings route

export default function AppSidebar() {
  const { state } = useSidebar();
  const navigate = useNavigate();
  const location = useLocation();
  // modal state removed; settings is now a routed page

  const isCollapsed = state === 'collapsed';

  // Keyboard shortcuts are handled by SidebarProvider's wrapper onKeyDown

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
            LibrAgent
          </span>
        </div>
      </SidebarHeader>

      <SidebarContent className={`flex-1 overflow-y-auto  terminal-scrollbar`}>
        {/* Agent Section */}
        <SidebarGroup>
          <SidebarGroupLabel className="text-sm font-semibold uppercase tracking-wide mb-2">
            Agent
          </SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <Link to="/agent">
                  <SidebarMenuButton
                    isActive={location.pathname.startsWith('/agent')}
                    tooltip="Start Agent"
                  >
                    <Bot size={16} />
                    <span>Start Agent</span>
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
                <Link to="/history">
                  <SidebarMenuButton
                    isActive={location.pathname === '/history'}
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
      </SidebarContent>
      <SidebarFooter className="border-t">
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              onClick={() => navigate('/settings')}
              tooltip="Settings"
              className={`transition-all duration-200`}
            >
              <Settings size={16} />
              {!isCollapsed && <span>Settings</span>}
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
        {!isCollapsed && (
          <div className="px-4 py-2 text-xs text-muted-foreground text-center">
            v{__APP_VERSION__}
          </div>
        )}
      </SidebarFooter>
    </Sidebar>
  );
}
