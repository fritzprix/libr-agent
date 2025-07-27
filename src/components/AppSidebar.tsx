import {
  BrainCircuit,
  History,
  MessageSquare,
  Plus,
  Settings,
  Users
} from "lucide-react";
import { useCallback, useEffect, useMemo } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import { useSessionContext } from "../context/SessionContext";
import SessionList from "./SessionList";
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
} from "./ui/sidebar";

interface AppSidebarProps {
  onOpenSettings: () => void;
  onOpenGroupCreationModal: () => void;
}

export default function AppSidebar({
  onOpenSettings,
  onOpenGroupCreationModal,
}: AppSidebarProps) {
  const { state, toggleSidebar } = useSidebar();
  const {
    select,
    sessions: sessionPages,
    current: currentSession,
    delete: deleteSession,
  } = useSessionContext();
  const location = useLocation();
  const navigate = useNavigate();

  const sessions = useMemo(
    () => (sessionPages ? sessionPages.flatMap((p) => p.items) : []),
    [sessionPages],
  );

  const handleLoadSession = useCallback(
    async (sessionId: string) => {
      select(sessionId);
      navigate("/chat");
    },
    [select, navigate],
  );

  const handleDeleteSession = useCallback(
    async (sessionId: string) => {
      await deleteSession(sessionId);
    },
    [deleteSession],
  );

  const isCollapsed = state === "collapsed";
  const currentView = location.pathname.substring(1); // Extract current view from path

  // Add keyboard shortcut for sidebar toggle
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.ctrlKey && event.key === "b") {
        event.preventDefault();
        toggleSidebar();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [toggleSidebar]);

  return (
    <Sidebar
      className="backdrop-blur-sm border-r shadow-xl"
      collapsible="icon"
      style={{
        '--sidebar-width-icon': '3.5rem',
      } as React.CSSProperties}
    >
      <SidebarHeader className="border-b">
        <div className="flex flex-row items-center justify-center gap-2 py-3">
          <BrainCircuit size={16} className="flex-shrink-0" />
          <span
            className={`font-medium text-sm whitespace-nowrap transition-all duration-300 ease-in-out ${isCollapsed
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
                <Link to="/chat">
                  <SidebarMenuButton
                    onClick={() => select()}
                    isActive={currentView === "chat"}
                    tooltip="New Chat"
                  >
                    <MessageSquare size={16} />
                    <span>New Chat</span>
                  </SidebarMenuButton>
                </Link>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        {/* Group Section */}
        <SidebarGroup>
          <SidebarGroupLabel className="text-sm font-semibold uppercase tracking-wide mb-2">
            Group
          </SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <Link to="/group">
                  <SidebarMenuButton
                    isActive={currentView === "group"}
                    tooltip="Create Group"
                  >
                    <Users size={16} />
                    <span>Create Group</span>
                  </SidebarMenuButton>
                </Link>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton
                  onClick={onOpenGroupCreationModal}
                  tooltip="New Group"
                >
                  <Plus size={16} />
                  <span>New Group</span>
                </SidebarMenuButton>
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
                    isActive={currentView === "history"}
                    tooltip="View History"
                  >
                    <History size={16} />
                    <span>View History</span>
                  </SidebarMenuButton>
                </Link>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        {/* Recent Sessions - only show if not in history view */}
        {currentView !== "history" && sessions.length > 0 && (
          <SidebarGroup>
            {!isCollapsed && (
              <SidebarGroupLabel className="text-xs font-semibold uppercase tracking-wider mb-2">
                Recent Sessions
              </SidebarGroupLabel>
            )}
            <SidebarGroupContent>
              <SessionList
                sessions={sessions.slice(0, 5)}
                currentSessionId={
                  currentView === "chat" ? currentSession?.id : undefined
                }
                onSelectSession={handleLoadSession}
                onDeleteSession={handleDeleteSession}
                showSearch={false}
                emptyMessage=""
              />
            </SidebarGroupContent>
          </SidebarGroup>
        )}
      </SidebarContent>

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