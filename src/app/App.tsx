import type { ReactNode } from 'react';
import { Route, Routes, Navigate } from 'react-router-dom';
import { lazy, Suspense, useEffect } from 'react';
import { Toaster } from '../components/ui/sonner';
import AppSidebar from '../components/layout/AppSidebar';
import { ThemeToggle } from '../components/common/ThemeToggle';
import { AppHeader } from '../components/layout/AppHeader';
import { SessionNotificationsBell } from '../components/layout/SessionNotificationsBell';
import { SidebarProvider } from '../components/ui/sidebar';
import { MCPServerProvider } from '../context/MCPServerContext';
import { MCPServerRegistryProvider } from '../context/MCPServerRegistryContext';
import { ModelOptionsProvider } from '../context/ModelProvider';
import { SkillsProvider } from '../context/SkillsContext';
import { DnDContextProvider } from '@/context/DnDContext';
import { LLMServiceProvider } from '@/context/LLMServiceContext';
import { AgentSessionListProvider } from '@/context/AgentSessionListContext';
import { GlobalEventProvider } from '@/context/GlobalEventContext';
import { UpdateProvider } from '@/context/UpdateContext';
import { useSettings } from '../context/SettingsContext';
import { markStartupMilestone } from '@/lib/performance/startup-metrics';
import { emitFrontendReadyOnce } from './frontend-ready';
import '../styles/globals.css';

// Lazy-load route components to reduce initial bundle and improve first paint
const AgentStartRoute = lazy(() => import('@/features/agent/AgentStartRoute'));
const AgentSessionRoute = lazy(
  () => import('@/features/agent/AgentSessionRoute'),
);
const AgentDraftChatView = lazy(
  () => import('@/features/agent/AgentDraftChatView'),
);
const AssistantListRoute = lazy(
  () => import('@/features/assistant/AssistantListRoute'),
);
const PlaybookList = lazy(() => import('@/features/playbook/List'));
const History = lazy(() => import('@/features/history/History'));
const Org = lazy(() => import('@/features/history/Org'));
const SettingsPage = lazy(() => import('@/features/settings/SettingsPage'));
const MCPServerPage = lazy(
  () => import('@/features/mcp-servers/MCPServerPage'),
);
const KnowledgePage = lazy(() => import('@/features/knowledge/KnowledgePage'));
const ScheduledTasksPage = lazy(
  () => import('@/features/scheduled-tasks/ScheduledTasksRoute'),
);

function StartupRouteMountMarker({
  routeName,
  children,
}: {
  routeName: string;
  children: ReactNode;
}) {
  useEffect(() => {
    markStartupMilestone('first-route-mounted', routeName);
  }, [routeName]);

  return <>{children}</>;
}

const FONT_MAP: Record<string, { sans: string; mono?: string }> = {
  Pretendard: {
    sans: "'Pretendard Variable', Pretendard, -apple-system, BlinkMacSystemFont, system-ui, sans-serif",
  },
  Inter: {
    sans: "'Inter Variable', Inter, -apple-system, BlinkMacSystemFont, system-ui, sans-serif",
  },
  'NanumSquare Neo': {
    sans: "'NanumSquareNeoVariable', 'NanumSquare Neo', -apple-system, BlinkMacSystemFont, system-ui, sans-serif",
  },
  D2Coding: {
    sans: "'D2Coding', monospace",
    mono: "'D2Coding', 'Cascadia Code', 'Source Code Pro', Menlo, Consolas, monospace",
  },
};

function App() {
  const { value: settings } = useSettings();

  useEffect(() => {
    markStartupMilestone('app-mounted');

    // Signal to backend that frontend is ready to receive events
    void emitFrontendReadyOnce();
  }, []);

  useEffect(() => {
    const frameId = window.requestAnimationFrame(() => {
      markStartupMilestone('first-frame-presented');
    });

    return () => {
      window.cancelAnimationFrame(frameId);
    };
  }, []);

  useEffect(() => {
    const fontName = settings.display.fontFamily || 'Pretendard';
    const config = FONT_MAP[fontName] || FONT_MAP.Pretendard;

    document.documentElement.style.setProperty('--app-font-sans', config.sans);
    if (config.mono) {
      document.documentElement.style.setProperty(
        '--app-font-mono',
        config.mono,
      );
    } else {
      document.documentElement.style.removeProperty('--app-font-mono');
    }
  }, [settings.display.fontFamily]);

  return (
    <div className="h-screen w-full">
      <UpdateProvider>
        <GlobalEventProvider>
          <SkillsProvider>
            <ModelOptionsProvider>
              <LLMServiceProvider>
                <MCPServerRegistryProvider>
                  <MCPServerProvider>
                    <AgentSessionListProvider>
                      <SidebarProvider className="h-full overflow-hidden">
                        <DnDContextProvider>
                          <AppSidebar />
                          {/* Main Content Area (children of AppSidebar) */}
                          <div className="flex flex-1 flex-col min-w-0">
                            <AppHeader>
                              <SessionNotificationsBell />
                              <ThemeToggle />
                            </AppHeader>
                            <div className="flex-1 w-full min-h-0 overflow-y-auto">
                              <Suspense
                                fallback={
                                  <div className="flex items-center justify-center h-full">
                                    Loading...
                                  </div>
                                }
                              >
                                <Routes>
                                  <Route
                                    path="/"
                                    element={<Navigate to="/agent" replace />}
                                  />
                                  <Route
                                    path="/agent"
                                    element={
                                      <StartupRouteMountMarker routeName="agent">
                                        <AgentStartRoute />
                                      </StartupRouteMountMarker>
                                    }
                                  />
                                  <Route
                                    path="/agent/draft"
                                    element={
                                      <StartupRouteMountMarker routeName="agent-draft">
                                        <AgentDraftChatView />
                                      </StartupRouteMountMarker>
                                    }
                                  />
                                  <Route
                                    path="/agent/:sessionId"
                                    element={
                                      <StartupRouteMountMarker routeName="agent-session">
                                        <AgentSessionRoute />
                                      </StartupRouteMountMarker>
                                    }
                                  />
                                  <Route
                                    path="/assistants"
                                    element={
                                      <StartupRouteMountMarker routeName="assistants">
                                        <AssistantListRoute />
                                      </StartupRouteMountMarker>
                                    }
                                  />
                                  <Route
                                    path="/playbooks"
                                    element={
                                      <StartupRouteMountMarker routeName="playbooks">
                                        <PlaybookList />
                                      </StartupRouteMountMarker>
                                    }
                                  />
                                  <Route
                                    path="/history"
                                    element={
                                      <StartupRouteMountMarker routeName="history">
                                        <History />
                                      </StartupRouteMountMarker>
                                    }
                                  />
                                  <Route
                                    path="/history/search"
                                    element={
                                      <StartupRouteMountMarker routeName="history-search">
                                        <History />
                                      </StartupRouteMountMarker>
                                    }
                                  />
                                  <Route
                                    path="/org"
                                    element={
                                      <StartupRouteMountMarker routeName="org">
                                        <Org />
                                      </StartupRouteMountMarker>
                                    }
                                  />
                                  <Route
                                    path="/settings"
                                    element={
                                      <StartupRouteMountMarker routeName="settings">
                                        <SettingsPage />
                                      </StartupRouteMountMarker>
                                    }
                                  />
                                  <Route
                                    path="/mcp-servers"
                                    element={
                                      <StartupRouteMountMarker routeName="mcp-servers">
                                        <MCPServerPage />
                                      </StartupRouteMountMarker>
                                    }
                                  />
                                  <Route
                                    path="/knowledge"
                                    element={
                                      <StartupRouteMountMarker routeName="knowledge">
                                        <KnowledgePage />
                                      </StartupRouteMountMarker>
                                    }
                                  />
                                  <Route
                                    path="/scheduled-tasks"
                                    element={
                                      <StartupRouteMountMarker routeName="scheduled-tasks">
                                        <ScheduledTasksPage />
                                      </StartupRouteMountMarker>
                                    }
                                  />
                                </Routes>
                              </Suspense>
                            </div>
                          </div>
                        </DnDContextProvider>
                      </SidebarProvider>
                      <Toaster position="top-right" />
                    </AgentSessionListProvider>
                  </MCPServerProvider>
                </MCPServerRegistryProvider>
              </LLMServiceProvider>
            </ModelOptionsProvider>
          </SkillsProvider>
        </GlobalEventProvider>
      </UpdateProvider>
    </div>
  );
}

export default App;
