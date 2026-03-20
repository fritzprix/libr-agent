import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Bot,
  BrainCircuit,
  History,
  Settings,
  Users,
  BookOpen,
  Blocks,
  Circle,
  Clock,
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
import { useAgentSessionListState } from '@/context/AgentSessionListContext';
import { useUpdateContext } from '@/context/UpdateContext';
import { buildChildrenMap } from '@/lib/session-utils';
import { cn } from '@/lib/utils';

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
  const { t } = useTranslation();
  const { state } = useSidebar();
  const location = useLocation();
  const isCollapsed = state === 'collapsed';
  const { status: updateStatus } = useUpdateContext();
  const hasUpdate = updateStatus === 'available';

  const { sessions } = useAgentSessionListState();

  /** Show up to 5 recent sessions with lightweight hierarchy cues. */
  const recentSessions = useMemo(() => {
    const statusPriority: Record<string, number> = {
      busy: 1,
      idle: 2,
      paused: 3,
      error: 4,
    };
    const sortByPriority = (
      a: (typeof sessions)[number],
      b: (typeof sessions)[number],
    ) => {
      const statusDiff =
        (statusPriority[a.status] ?? 9) - (statusPriority[b.status] ?? 9);
      if (statusDiff !== 0) return statusDiff;
      return (
        (b.updatedAt ?? b.createdAt).getTime() -
        (a.updatedAt ?? a.createdAt).getTime()
      );
    };

    const sortedSessions = [...sessions].sort(sortByPriority);
    const sessionById = new Map(
      sortedSessions.map((session) => [session.id, session]),
    );
    const childrenByParent = buildChildrenMap(sortedSessions);
    const rows: Array<{
      session: (typeof sessions)[number];
      nestingLevel: number;
    }> = [];
    const visited = new Set<string>();

    const pushSession = (
      session: (typeof sessions)[number],
      nestingLevel: number,
    ) => {
      if (visited.has(session.id) || rows.length >= 5) {
        return;
      }

      visited.add(session.id);
      rows.push({ session, nestingLevel });

      const children = (childrenByParent.get(session.id) || []).sort(
        sortByPriority,
      );
      children.forEach((child) => {
        pushSession(child, Math.min(nestingLevel + 1, 2));
      });
    };

    sortedSessions
      .filter(
        (session) =>
          !session.parentSessionId || !sessionById.has(session.parentSessionId),
      )
      .forEach((root) => {
        pushSession(root, 0);
      });

    sortedSessions.forEach((session) => {
      pushSession(session, session.parentSessionId ? 1 : 0);
    });

    return rows;
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
                  tooltip={t('sidebar.chat')}
                >
                  <Link to="/agent">
                    <Bot size={16} />
                    <span>{t('sidebar.chat')}</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        {/* Library Section */}
        <SidebarGroup>
          <SidebarGroupLabel className="text-sm font-semibold uppercase tracking-wide mb-2">
            {t('sidebar.library')}
          </SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  isActive={location.pathname === '/assistants'}
                  tooltip={t('sidebar.assistants')}
                >
                  <Link to="/assistants">
                    <Users size={16} />
                    <span>{t('sidebar.assistants')}</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  isActive={location.pathname === '/playbooks'}
                  tooltip={t('sidebar.playbooks')}
                >
                  <Link to="/playbooks">
                    <BookOpen size={16} />
                    <span>{t('sidebar.playbooks')}</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  isActive={location.pathname === '/mcp-servers'}
                  tooltip={t('sidebar.extensions')}
                >
                  <Link to="/mcp-servers">
                    <Blocks size={16} />
                    <span>{t('sidebar.extensions')}</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  isActive={location.pathname.startsWith('/history')}
                  tooltip={t('sidebar.history')}
                >
                  <Link to="/history">
                    <History size={16} />
                    <span>{t('sidebar.history')}</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  isActive={location.pathname === '/scheduled-tasks'}
                  tooltip={t('sidebar.scheduledTasks')}
                >
                  <Link to="/scheduled-tasks">
                    <Clock size={16} />
                    <span>{t('sidebar.scheduledTasks')}</span>
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
              {t('sidebar.recentSessions')}
            </SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                {recentSessions.map(({ session, nestingLevel }) => (
                  <SidebarMenuItem key={session.id}>
                    <SidebarMenuButton
                      asChild
                      isActive={location.pathname === `/agent/${session.id}`}
                      tooltip={
                        session.name ||
                        `${t('sidebar.session')} ${session.id.slice(0, 8)}`
                      }
                    >
                      <Link
                        to={`/agent/${session.id}`}
                        className={cn(
                          'gap-2',
                          nestingLevel > 0 && 'text-muted-foreground',
                        )}
                        style={{ paddingLeft: `${nestingLevel * 12}px` }}
                      >
                        <StatusDot status={session.status} />
                        {nestingLevel > 0 && (
                          <span className="text-[10px]" aria-hidden="true">
                            ↳
                          </span>
                        )}
                        <span className="truncate text-xs">
                          {session.name ||
                            `${t('sidebar.session')} ${session.id.slice(0, 8)}`}
                        </span>
                      </Link>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                ))}
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
              tooltip={t('sidebar.settings')}
              className={`transition-all duration-200`}
              isActive={location.pathname === '/settings'}
            >
              <Link to="/settings" className="relative">
                <Settings size={16} />
                {hasUpdate && (
                  <span className="absolute -top-1 -right-1 w-2 h-2 rounded-full bg-destructive" />
                )}
                {!isCollapsed && <span>{t('sidebar.settings')}</span>}
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
