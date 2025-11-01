import { Route, Routes } from 'react-router-dom';
import { Suspense, lazy } from 'react';
import AppSidebar from '../components/layout/AppSidebar';

// Lazy-load route components to reduce initial bundle and improve first paint
const ChatContainer = lazy(() => import('@/features/chat/ChatContainer'));
const AssistantList = lazy(() => import('@/features/assistant/List'));
const History = lazy(() => import('@/features/history/History'));
const SettingsPage = lazy(() => import('@/features/settings/SettingsPage'));

import { Toaster } from 'sonner';
import { ThemeToggle } from '../components/common/ThemeToggle';
import { AppHeader } from '../components/layout/AppHeader';
import { SidebarProvider } from '../components/ui/sidebar';
import { AssistantContextProvider } from '../context/AssistantContext';
import { MCPServerProvider } from '../context/MCPServerContext';
import { MCPServerRegistryProvider } from '../context/MCPServerRegistryContext';
import { ModelOptionsProvider } from '../context/ModelProvider';
import { SessionContextProvider } from '../context/SessionContext';
import { SessionHistoryProvider } from '../context/SessionHistoryContext';
import { SettingsProvider } from '../context/SettingsContext';
import '../styles/globals.css';
import './App.css';
import { ResourceAttachmentProvider } from '@/context/ResourceAttachmentContext';
import { BuiltInToolProvider } from '@/features/tools';
import { SystemPromptProvider } from '@/context/SystemPromptContext';
import { DnDContextProvider } from '@/context/DnDContext';

function App() {
  return (
    <div className="h-screen w-full">
      <SettingsProvider>
        <MCPServerRegistryProvider>
          <MCPServerProvider>
            <SystemPromptProvider>
              <AssistantContextProvider>
                <SessionContextProvider>
                  <BuiltInToolProvider>
                    <SessionHistoryProvider>
                      <ResourceAttachmentProvider>
                        <ModelOptionsProvider>
                          <SidebarProvider>
                            <DnDContextProvider>
                              <AppSidebar />
                              {/* Main Content Area (children of AppSidebar) */}
                              <div className="flex flex-1 flex-col min-w-0">
                                <AppHeader>
                                  <ThemeToggle />
                                </AppHeader>
                                <div className="flex-1 w-full min-h-0">
                                  <Suspense
                                    fallback={
                                      <div className="p-4 text-sm text-muted-foreground">
                                        Loading…
                                      </div>
                                    }
                                  >
                                    <Routes>
                                      <Route
                                        path="/"
                                        element={<ChatContainer />}
                                      />
                                      <Route
                                        path="/chat/single"
                                        element={<ChatContainer />}
                                      />
                                      <Route
                                        path="/assistants"
                                        element={<AssistantList />}
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
                                    </Routes>
                                  </Suspense>
                                </div>
                              </div>
                            </DnDContextProvider>
                          </SidebarProvider>
                          <Toaster />
                        </ModelOptionsProvider>
                      </ResourceAttachmentProvider>
                    </SessionHistoryProvider>
                  </BuiltInToolProvider>
                </SessionContextProvider>
              </AssistantContextProvider>
            </SystemPromptProvider>
          </MCPServerProvider>
        </MCPServerRegistryProvider>
      </SettingsProvider>
    </div>
  );
}

export default App;
