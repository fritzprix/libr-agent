import { useState } from 'react';
import { Route, Routes } from 'react-router-dom';
import AppSidebar from '../components/layout/AppSidebar';
import AssistantGroupDetailList from '../features/assistant/AssistantGroupDetailList';

import GroupChatContainer from '@/features/chat/GroupChatContainer';
import SingleChatContainer from '@/features/chat/SingleChatContainer';
import { Toaster } from 'sonner';
import { ThemeToggle } from '../components/common/ThemeToggle';
import { AppHeader } from '../components/layout/AppHeader';
import { SidebarProvider } from '../components/ui/sidebar';
import { AssistantContextProvider } from '../context/AssistantContext';
import { AssistantExtensionProvider } from '../context/AssistantExtensionContext';
import { AssistantGroupProvider } from '../context/AssistantGroupContext';
import { LocalToolProvider } from '../context/LocalToolContext';
import { MCPServerProvider } from '../context/MCPServerContext';
import { WebMCPProvider } from '../context/WebMCPContext';
import { UnifiedMCPProvider } from '../context/UnifiedMCPContext';
import { ModelOptionsProvider } from '../context/ModelProvider';
import { SchedulerProvider } from '../context/SchedulerContext';
import { SessionContextProvider } from '../context/SessionContext';
import { SessionHistoryProvider } from '../context/SessionHistoryContext';
import { SettingsProvider } from '../context/SettingsContext';
import History from '../features/history/History';
import SettingsModal from '../features/settings/SettingsModal';
import '../styles/globals.css';
import './App.css';
import AssistantList from '@/features/assistant/List';
import WebMCPDemo from '@/features/tools/WebMCPDemo';

function App() {
  const [isSettingsModalOpen, setIsSettingsModalOpen] = useState(false);

  return (
    <div className="h-screen w-full">
      <SettingsProvider>
        <SchedulerProvider>
          <AssistantGroupProvider>
            <AssistantContextProvider>
              <SessionContextProvider>
                <SessionHistoryProvider>
                  <ModelOptionsProvider>
                    <LocalToolProvider>
                      <AssistantExtensionProvider>
                        <MCPServerProvider>
                          <WebMCPProvider
                            servers={['calculator', 'filesystem', 'file-store']}
                            autoLoad={true}
                          >
                            <UnifiedMCPProvider>
                              <SidebarProvider>
                                <div className="flex h-screen w-full">
                                  {/* Sidebar */}
                                  <AppSidebar
                                    onOpenSettings={() =>
                                      setIsSettingsModalOpen(true)
                                    }
                                  />

                                  {/* Main Content Area */}
                                  <div className="flex flex-1 flex-col min-w-0">
                                    <AppHeader>
                                      <ThemeToggle />
                                    </AppHeader>
                                    <div className="flex-1 overflow-auto w-full">
                                      <Routes>
                                        <Route
                                          path="/"
                                          element={<SingleChatContainer />}
                                        />
                                        <Route
                                          path="/chat/single"
                                          element={<SingleChatContainer />}
                                        />
                                        <Route
                                          path="/chat/group"
                                          element={<GroupChatContainer />}
                                        />
                                        <Route
                                          path="/chat/flow"
                                          element={<SingleChatContainer />}
                                        />
                                        <Route
                                          path="/assistants"
                                          element={<AssistantList />}
                                        />
                                        <Route
                                          path="/assistants/groups"
                                          element={<AssistantGroupDetailList />}
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
                                          path="/tools/webmcp-demo"
                                          element={<WebMCPDemo />}
                                        />
                                      </Routes>
                                    </div>
                                  </div>
                                </div>
                              </SidebarProvider>
                              <SettingsModal
                                isOpen={isSettingsModalOpen}
                                onClose={() => setIsSettingsModalOpen(false)}
                              />
                              <Toaster />
                            </UnifiedMCPProvider>
                          </WebMCPProvider>
                        </MCPServerProvider>
                      </AssistantExtensionProvider>
                    </LocalToolProvider>
                  </ModelOptionsProvider>
                </SessionHistoryProvider>
              </SessionContextProvider>
            </AssistantContextProvider>
          </AssistantGroupProvider>
        </SchedulerProvider>
      </SettingsProvider>
    </div>
  );
}

export default App;
