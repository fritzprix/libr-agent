import { Route, Routes, Navigate } from 'react-router-dom';
import { lazy, Suspense, useEffect } from 'react';
import { emit } from '@tauri-apps/api/event';
import { Toaster } from 'sonner';
import AppSidebar from '../components/layout/AppSidebar';
import { ThemeToggle } from '../components/common/ThemeToggle';
import { AppHeader } from '../components/layout/AppHeader';
import { SessionNotificationsBell } from '../components/layout/SessionNotificationsBell';
import { SidebarProvider } from '../components/ui/sidebar';
import { AssistantContextProvider } from '../context/AssistantContext';
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
import '../styles/globals.css';

// Lazy-load route components to reduce initial bundle and improve first paint
const AgentContainer = lazy(() => import('@/features/agent'));
const AgentDraftChatView = lazy(
  () => import('@/features/agent/AgentDraftChatView'),
);
const AssistantList = lazy(() => import('@/features/assistant/List'));
const PlaybookList = lazy(() => import('@/features/playbook/List'));
const History = lazy(() => import('@/features/history/History'));
const SettingsPage = lazy(() => import('@/features/settings/SettingsPage'));
const MCPServerPage = lazy(
  () => import('@/features/mcp-servers/MCPServerPage'),
);
const ScheduledTasksPage = lazy(() =>
  import('@/features/scheduled-tasks/ScheduledTasksPage').then((m) => ({
    default: m.ScheduledTasksPage,
  })),
);

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
    // Signal to backend that frontend is ready to receive events
    emit('frontend-ready').catch((e) => {
      console.error('Failed to emit frontend-ready event', e);
    });
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
                    <AssistantContextProvider>
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
                                      element={<AgentContainer />}
                                    />
                                    <Route
                                      path="/agent/draft"
                                      element={<AgentDraftChatView />}
                                    />
                                    <Route
                                      path="/agent/:sessionId"
                                      element={<AgentContainer />}
                                    />
                                    <Route
                                      path="/assistants"
                                      element={<AssistantList />}
                                    />
                                    <Route
                                      path="/playbooks"
                                      element={<PlaybookList />}
                                    />
                                    <Route
                                      path="/history"
                                      element={<History />}
                                    />
                                    <Route
                                      path="/history/search"
                                      element={<History />}
                                    />
                                    <Route
                                      path="/settings"
                                      element={<SettingsPage />}
                                    />
                                    <Route
                                      path="/mcp-servers"
                                      element={<MCPServerPage />}
                                    />
                                    <Route
                                      path="/scheduled-tasks"
                                      element={<ScheduledTasksPage />}
                                    />
                                  </Routes>
                                </Suspense>
                              </div>
                            </div>
                          </DnDContextProvider>
                        </SidebarProvider>
                        <Toaster />
                      </AgentSessionListProvider>
                    </AssistantContextProvider>
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
