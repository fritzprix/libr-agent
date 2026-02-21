import { useMemo } from 'react';
import {
  Bot,
  BrainCircuit,
  History,
  Settings,
  Users,
  BookOpen,
  Blocks,
  Circle,
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
import {
  useAgentSessionListState,
  useAgentSessionListActions,
} from '@/context/AgentSessionListContext';
import { useEffect } from 'react';

/** Maps session status to a semantically meaningful dot */
function StatusDot({ status }: { status: string }) {
  if (status === 'busy') {
    // Pulsing to signal active work
    return (
      <Circle
        size={8}
        className="fill-primary text-primary flex-shrink-0 animate-pulse"
      />
    );
  }
  if (status === 'error') {
    return (
      <Circle
        size={8}
        className="fill-destructive text-destructive flex-shrink-0"
      />
    );
  }
  // idle and paused: dimmed — just resting
  return (
    <Circle
      size={8}
      className="fill-muted-foreground text-muted-foreground flex-shrink-0 opacity-40"
    />
  );
}

export default function AppSidebar() {
  const { state } = useSidebar();
  const location = useLocation();
  const isCollapsed = state === 'collapsed';

  const { sessions } = useAgentSessionListState();
  const { loadSessions } = useAgentSessionListActions();

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  /** Show up to 5 sessions: busy first, then by most recent */
  const recentSessions = useMemo(() => {
    const statusPriority: Record<string, number> = {
      busy: 1,
      idle: 2,
      paused: 3,
      error: 4,
    };
    return [...sessions]
      .sort((a, b) => {
        const statusDiff =
          (statusPriority[a.status] ?? 9) - (statusPriority[b.status] ?? 9);
        if (statusDiff !== 0) return statusDiff;
        return (
          (b.updatedAt ?? b.createdAt).getTime() -
          (a.updatedAt ?? a.createdAt).getTime()
        );
      })
      .slice(0, 5);
  }, [sessions]);

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

      <SidebarContent className={`flex-1 overflow-y-auto terminal-scrollbar`}>
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

        {/* Recent Sessions – only visible when sidebar is expanded */}
        {!isCollapsed && recentSessions.length > 0 && (
          <SidebarGroup>
            <SidebarGroupLabel className="text-sm font-semibold uppercase tracking-wide mb-2">
              Recent Sessions
            </SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                {recentSessions.map((session) => (
                  <SidebarMenuItem key={session.id}>
                    <SidebarMenuButton
                      asChild
                      isActive={location.pathname === `/agent/${session.id}`}
                      tooltip={session.name || session.id.slice(0, 8)}
                    >
                      <Link to={`/agent/${session.id}`} className="gap-2">
                        <StatusDot status={session.status} />
                        <span className="truncate text-xs">
                          {session.name || `Session ${session.id.slice(0, 8)}`}
                        </span>
                      </Link>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                ))}
                <SidebarMenuItem>
                  <SidebarMenuButton
                    asChild
                    isActive={location.pathname === '/history'}
                    tooltip="All Sessions"
                  >
                    <Link
                      to="/history"
                      className="text-muted-foreground hover:text-foreground"
                    >
                      <History size={14} />
                      <span className="text-xs">See all sessions →</span>
                    </Link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        )}
        {/* History icon visible when collapsed, so history is still reachable */}
        {isCollapsed && (
          <SidebarGroup>
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
                    </Link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        )}
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
