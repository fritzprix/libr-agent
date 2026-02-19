import {
  Bot,
  BrainCircuit,
  History,
  Settings,
  Users,
  BookOpen,
  Blocks,
} from 'lucide-react';
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

export default function AppSidebar() {
  const { state } = useSidebar();
  const location = useLocation();
  // modal state removed; settings is now a routed page

  const isCollapsed = state === 'collapsed';

  // Keyboard shortcuts are handled by SidebarProvider's wrapper onKeyDown

  return (
    <Sidebar className="backdrop-blur-sm border-r shadow-xl" collapsible="icon">
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
        {/* Main Section */}
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  isActive={location.pathname.startsWith('/agent')}
                  tooltip="Chat"
                >
                  <Link to="/agent">
                    <Bot size={16} />
                    <span>Chat</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        {/* Library Section */}
        <SidebarGroup>
          <SidebarGroupLabel className="text-sm font-semibold uppercase tracking-wide mb-2">
            Library
          </SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  isActive={location.pathname === '/assistants'}
                  tooltip="Assistants"
                >
                  <Link to="/assistants">
                    <Users size={16} />
                    <span>Assistants</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  isActive={location.pathname === '/playbooks'}
                  tooltip="Playbooks"
                >
                  <Link to="/playbooks">
                    <BookOpen size={16} />
                    <span>Playbooks</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  isActive={location.pathname === '/mcp-servers'}
                  tooltip="Extensions"
                >
                  <Link to="/mcp-servers">
                    <Blocks size={16} />
                    <span>Extensions</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        {/* Activity Section */}
        <SidebarGroup>
          <SidebarGroupLabel className="text-sm font-semibold uppercase tracking-wide mb-2">
            Activity
          </SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  isActive={location.pathname === '/history'}
                  tooltip="History"
                >
                  <Link to="/history">
                    <History size={16} />
                    <span>History</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter className="border-t">
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              asChild
              tooltip="Settings"
              className={`transition-all duration-200`}
              isActive={location.pathname === '/settings'}
            >
              <Link to="/settings">
                <Settings size={16} />
                {!isCollapsed && <span>Settings</span>}
              </Link>
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
