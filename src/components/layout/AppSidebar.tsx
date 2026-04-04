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

      const children = childrenByParent.get(session.id) || [];
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
      <SidebarHeader className="h-16 border-b shrink-0 p-0 flex flex-row items-center">
        <div
          className={cn(
            'flex flex-row items-center justify-center gap-2 transition-all duration-300 ease-in-out w-full',
            isCollapsed ? 'px-2' : 'px-4',
          )}
        >
          <BrainCircuit
            size={isCollapsed ? 24 : 32}
            className="flex-shrink-0 text-primary"
          />
          <span
            className={cn(
              'font-medium text-2xl whitespace-nowrap transition-all duration-300 ease-in-out',
              isCollapsed
                ? 'opacity-0 w-0 overflow-hidden'
                : 'opacity-100 w-auto',
            )}
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
                  <Link to="/agent" className="flex w-full items-center gap-2">
                    <Bot className="shrink-0" />
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
                  <Link
                    to="/assistants"
                    className="flex w-full items-center gap-2"
                  >
                    <Users className="shrink-0" />
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
                  <Link
                    to="/playbooks"
                    className="flex w-full items-center gap-2"
                  >
                    <BookOpen className="shrink-0" />
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
                  <Link
                    to="/mcp-servers"
                    className="flex w-full items-center gap-2"
                  >
                    <Blocks className="shrink-0" />
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
                  <Link
                    to="/history"
                    className="flex w-full items-center gap-2"
                  >
                    <History className="shrink-0" />
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
                  <Link
                    to="/scheduled-tasks"
                    className="flex w-full items-center gap-2"
                  >
                    <Clock className="shrink-0" />
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
                          'flex w-full items-center gap-2',
                          nestingLevel > 0 && 'text-muted-foreground',
                        )}
                        style={{ paddingLeft: `${nestingLevel * 12}px` }}
                      >
                        <StatusDot status={session.status} />
                        {nestingLevel > 0 && (
                          <span
                            className="text-[10px] shrink-0"
                            aria-hidden="true"
                          >
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

      <SidebarFooter className="border-t p-2">
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              asChild
              tooltip={t('sidebar.settings')}
              className="transition-all duration-200"
              isActive={location.pathname === '/settings'}
            >
              <Link
                to="/settings"
                className="flex w-full items-center justify-between gap-2"
              >
                <div className="flex items-center gap-2 overflow-hidden">
                  <Settings className="shrink-0" />
                  {!isCollapsed && (
                    <span className="truncate">{t('sidebar.settings')}</span>
                  )}
                </div>

                {hasUpdate && isCollapsed && (
                  <span className="absolute left-4 top-2 h-2 w-2 rounded-full bg-destructive" />
                )}

                {!isCollapsed && (
                  <div className="flex items-center gap-2 shrink-0">
                    {hasUpdate && (
                      <span className="h-2 w-2 rounded-full bg-destructive animate-pulse" />
                    )}
                    <span className="text-[10px] font-mono text-muted-foreground bg-muted px-1.5 py-0.5 rounded-md border border-border/50">
                      v{__APP_VERSION__}
                    </span>
                  </div>
                )}
              </Link>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  );
}
