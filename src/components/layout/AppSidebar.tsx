import { useCallback, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Bot,
  BrainCircuit,
  History,
  BookmarkCheck,
  Network,
  Settings,
  Users,
  BookOpen,
  Blocks,
  Circle,
  Clock,
  Database,
  ChevronDown,
  ChevronRight,
  Loader2,
} from 'lucide-react';
import { Link, useLocation } from 'react-router-dom';
import { toast } from 'sonner';
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
import { Badge } from '../ui/badge';
import {
  useAgentSessionListActions,
  useAgentSessionListState,
} from '@/context/AgentSessionListContext';
import { useUpdateContext } from '@/context/UpdateContext';
import { useInfiniteScroll } from '@/features/agent/components/use-session-scroll';
import { useKnownDirectChildCounts } from '@/features/agent/components/use-known-direct-child-counts';
import { getLogger } from '@/lib/logger';
import { cn } from '@/lib/utils';
import { buildSidebarSessionRows } from './sidebar-recent-sessions';

const logger = getLogger('AppSidebar');

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
  if (status === 'queued') {
    return (
      <Circle
        size={8}
        className="fill-warning text-warning flex-shrink-0 animate-pulse"
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

  const {
    sessions,
    hasMoreSessions,
    isLoadingMoreSessions,
    loadingChildrenParentIds,
  } = useAgentSessionListState();
  const { loadMoreSessions, ensureChildrenLoaded } =
    useAgentSessionListActions();

  const recentSessionsRootRef = useRef<HTMLDivElement | null>(null);
  const loadMoreSentinelRef = useRef<HTMLDivElement | null>(null);
  const [expandedSessionIds, setExpandedSessionIds] = useState<Set<string>>(
    () => new Set(),
  );

  const knownDirectChildCountByParentId = useKnownDirectChildCounts(
    sessions,
    hasMoreSessions,
  );

  const bookmarkedCount = useMemo(
    () => sessions.filter((session) => session.isBookmarked === true).length,
    [sessions],
  );

  const recentSessions = useMemo(
    () =>
      buildSidebarSessionRows(
        sessions,
        expandedSessionIds,
        knownDirectChildCountByParentId,
      ),
    [sessions, expandedSessionIds, knownDirectChildCountByParentId],
  );

  const handleLoadMore = useCallback(() => {
    void loadMoreSessions().catch((error) => {
      logger.error('Failed to load more sessions', error);
      toast.error(
        t(
          'sessionHistory.toasts.loadMoreFailed',
          'Failed to load more sessions',
        ),
      );
    });
  }, [loadMoreSessions, t]);

  const handleToggleExpand = useCallback(
    (sessionId: string) => {
      setExpandedSessionIds((previous) => {
        const next = new Set(previous);
        if (next.has(sessionId)) {
          next.delete(sessionId);
        } else {
          next.add(sessionId);
          void ensureChildrenLoaded(sessionId).catch((error) => {
            logger.error('Failed to load session children', {
              sessionId,
              error,
            });
            toast.error(
              t(
                'sessionHistory.toasts.loadChildrenFailed',
                'Failed to load child sessions',
              ),
            );
          });
        }
        return next;
      });
    },
    [ensureChildrenLoaded, t],
  );

  useInfiniteScroll({
    rootRef: recentSessionsRootRef,
    loadMoreSentinelRef,
    hasMoreSessions,
    isLoadingMoreSessions,
    onLoadMore: handleLoadMore,
    displayRowsLength: recentSessions.length,
  });

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
            {t('appName')}
          </span>
        </div>
      </SidebarHeader>

      <SidebarContent className="flex min-h-0 flex-1 flex-col overflow-hidden">
        {/* Main Section */}
        <SidebarGroup className="shrink-0">
          <SidebarGroupContent>
            <SidebarMenu>
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
                  isActive={
                    location.pathname.startsWith('/history') &&
                    location.hash === '#bookmarked-sessions'
                  }
                  tooltip={t('sidebar.bookmarked', 'Bookmarked')}
                >
                  <Link
                    to="/history#bookmarked-sessions"
                    className="flex w-full items-center justify-between gap-2"
                  >
                    <div className="flex min-w-0 items-center gap-2">
                      <BookmarkCheck className="shrink-0" />
                      <span className="truncate">
                        {t('sidebar.bookmarked', 'Bookmarked')}
                      </span>
                    </div>
                    {!isCollapsed &&
                      bookmarkedCount > 0 &&
                      !hasMoreSessions && (
                        <Badge
                          variant="secondary"
                          className="h-5 shrink-0 px-1.5"
                        >
                          {bookmarkedCount}
                        </Badge>
                      )}
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
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
        <SidebarGroup className="shrink-0">
          <SidebarGroupLabel className="text-sm font-semibold uppercase tracking-wide mb-2">
            {t('sidebar.library')}
          </SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  isActive={location.pathname.startsWith('/knowledge')}
                  tooltip={t('sidebar.knowledge')}
                >
                  <Link
                    to="/knowledge"
                    className="flex w-full items-center gap-2"
                  >
                    <Database className="shrink-0" />
                    <span>{t('sidebar.knowledge')}</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
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
                  isActive={location.pathname.startsWith('/org')}
                  tooltip={t('sidebar.org')}
                >
                  <Link to="/org" className="flex w-full items-center gap-2">
                    <Network className="shrink-0" />
                    <span>{t('sidebar.org')}</span>
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
        {!isCollapsed && sessions.length > 0 && (
          <SidebarGroup className="flex min-h-0 flex-1 flex-col">
            <SidebarGroupLabel className="mb-2 shrink-0 text-sm font-semibold uppercase tracking-wide">
              {t('sidebar.recentSessions')}
            </SidebarGroupLabel>
            <div className="min-h-0 flex-1 overflow-y-auto terminal-scrollbar">
              {/*
                rootRef must be a descendant of the overflow container —
                useInfiniteScroll's findScrollParent starts at parentElement.
              */}
              <div ref={recentSessionsRootRef}>
                <SidebarMenu>
                  {recentSessions.map(
                    ({
                      session,
                      nestingLevel,
                      hasExpandableChildren,
                      isExpanded,
                    }) => {
                      const isLoadingChildren = loadingChildrenParentIds.has(
                        session.id,
                      );

                      return (
                        <SidebarMenuItem key={session.id}>
                          <SidebarMenuButton
                            asChild
                            isActive={
                              location.pathname === `/agent/${session.id}`
                            }
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
                              style={{
                                paddingLeft: `${nestingLevel * 12}px`,
                              }}
                            >
                              {hasExpandableChildren ? (
                                <button
                                  type="button"
                                  className="inline-flex h-4 w-4 shrink-0 items-center justify-center rounded-sm text-muted-foreground hover:text-foreground"
                                  aria-expanded={isExpanded}
                                  aria-busy={isLoadingChildren}
                                  aria-label={
                                    isLoadingChildren
                                      ? t(
                                          'sidebar.loadingSessionChildren',
                                          'Loading session children',
                                        )
                                      : isExpanded
                                        ? t(
                                            'sidebar.collapseSession',
                                            'Collapse session children',
                                          )
                                        : t(
                                            'sidebar.expandSession',
                                            'Expand session children',
                                          )
                                  }
                                  onClick={(event) => {
                                    event.preventDefault();
                                    event.stopPropagation();
                                    handleToggleExpand(session.id);
                                  }}
                                >
                                  {isLoadingChildren ? (
                                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                                  ) : isExpanded ? (
                                    <ChevronDown className="h-3.5 w-3.5" />
                                  ) : (
                                    <ChevronRight className="h-3.5 w-3.5" />
                                  )}
                                </button>
                              ) : (
                                <span className="inline-block h-4 w-4 shrink-0" />
                              )}
                              <StatusDot status={session.status} />
                              <span className="truncate text-xs">
                                {session.name ||
                                  `${t('sidebar.session')} ${session.id.slice(0, 8)}`}
                              </span>
                              {session.isBookmarked && (
                                <BookmarkCheck className="h-3.5 w-3.5 shrink-0 text-warning" />
                              )}
                            </Link>
                          </SidebarMenuButton>
                        </SidebarMenuItem>
                      );
                    },
                  )}
                </SidebarMenu>
                <div ref={loadMoreSentinelRef} className="h-px w-full" />
                {isLoadingMoreSessions && (
                  <div className="flex items-center justify-center gap-2 py-2 text-xs text-muted-foreground">
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    <span>
                      {t('sessionHistory.loadingMore', 'Loading more...')}
                    </span>
                  </div>
                )}
              </div>
            </div>
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
