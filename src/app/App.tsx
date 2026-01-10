import { Route, Routes, Navigate } from 'react-router-dom';
import { lazy, Suspense } from 'react';
import AppSidebar from '../components/layout/AppSidebar';

// Lazy-load route components to reduce initial bundle and improve first paint
const AgentContainer = lazy(() => import('@/features/agent'));
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
import { WebMCPProvider } from '@/context/WebMCPContext';
import { WebMCPServiceRegistry } from '@/features/tools/WebMCPServiceRegistry';
import { BrowserToolProvider } from '@/features/tools/BrowserToolProvider';
import { RustMCPToolProvider } from '@/features/tools/RustMCPToolProvider';
import { LLMServiceProvider } from '@/context/LLMServiceContext';
import { AgentSessionListProvider } from '@/context/AgentSessionListContext';

function App() {
  return (
    <div className="h-screen w-full">
      <SettingsProvider>
        <ModelOptionsProvider>
          <SystemPromptProvider>
            <LLMServiceProvider>
              <WebMCPProvider>
                <MCPServerRegistryProvider>
                  <MCPServerProvider>
                    <AssistantContextProvider>
                      <SessionContextProvider>
                        <AgentSessionListProvider>
                          <BuiltInToolProvider>
                            <WebMCPServiceRegistry
                              servers={[
                                // All builtin servers migrated to Rust backend
                                // Agent V2 uses Rust MCPServiceProxy directly
                              ]}
                            />
                            <BrowserToolProvider />
                            <RustMCPToolProvider />
                            <SessionHistoryProvider>
                              <ResourceAttachmentProvider>
                                <SidebarProvider className="h-full overflow-hidden">
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
                                            <div className="flex items-center justify-center h-full">
                                              Loading...
                                            </div>
                                          }
                                        >
                                          <Routes>
                                            <Route
                                              path="/"
                                              element={
                                                <Navigate to="/agent" replace />
                                              }
                                            />
                                            <Route
                                              path="/agent"
                                              element={<AgentContainer />}
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
                              </ResourceAttachmentProvider>
                            </SessionHistoryProvider>
                          </BuiltInToolProvider>
                        </AgentSessionListProvider>
                      </SessionContextProvider>
                    </AssistantContextProvider>
                  </MCPServerProvider>
                </MCPServerRegistryProvider>
              </WebMCPProvider>
            </LLMServiceProvider>
          </SystemPromptProvider>
        </ModelOptionsProvider>
      </SettingsProvider>
    </div>
  );
}

export default App;
