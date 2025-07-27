import { useState } from 'react';
import { Route, Routes } from 'react-router-dom';
import AppSidebar from '../components/layout/AppSidebar';
import AssistantDetailList from '../features/assistant/AssistantDetailList';
import AssistantGroupDetailList from '../features/assistant/AssistantGroupDetailList';
import StartGroupChatView from '../features/chat/StartGroupChatView';

import { ThemeToggle } from '../components/common/ThemeToggle';
import { AppHeader } from '../components/layout/AppHeader';
import { SidebarProvider } from '../components/ui/sidebar';
import { AssistantContextProvider } from '../context/AssistantContext';
import { AssistantGroupProvider } from '../context/AssistantGroupContext';
import { LocalToolProvider } from '../context/LocalToolContext';
import { MCPServerProvider } from '../context/MCPServerContext';
import { ModelOptionsProvider } from '../context/ModelProvider';
import { SessionContextProvider } from '../context/SessionContext';
import { SessionHistoryProvider } from '../context/SessionHistoryContext';
import { SettingsProvider } from '../context/SettingsContext';
import History from '../features/history/History';
import SettingsModal from '../features/settings/SettingsModal';
import { WeatherTool } from '../features/tools/WeatherTool';
import "../styles/globals.css";
import "./App.css";
import SingleChatContainer from '@/features/chat/SingleChatContainer';
import GroupChatContainer from '@/features/chat/GroupChatContainer';

function App() {
  const [isSettingsModalOpen, setIsSettingsModalOpen] = useState(false);

  return (
    <div className="h-screen w-full">
      <SettingsProvider>
        <AssistantGroupProvider>
          <AssistantContextProvider>
            <SessionContextProvider>
              <SessionHistoryProvider>
                <ModelOptionsProvider>
                  <MCPServerProvider>
                    <LocalToolProvider>
                      <WeatherTool />
                      <SidebarProvider>
                        <div className="flex h-screen w-full">
                          {/* Sidebar */}
                          <AppSidebar
                            onOpenSettings={() => setIsSettingsModalOpen(true)}
                          />

                          {/* Main Content Area */}
                          <div className="flex flex-1 flex-col min-w-0">
                            <AppHeader>
                              <ThemeToggle />
                            </AppHeader>
                            <div className="flex-1 overflow-auto w-full">
                              <Routes>
                                <Route path="/" element={<SingleChatContainer />} />
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
                                  element={<AssistantDetailList />}
                                />
                                <Route
                                  path="/assistants/groups"
                                  element={<AssistantGroupDetailList />}
                                />
                                <Route path="/history" element={<History />} />
                                <Route
                                  path="/history/search"
                                  element={<History />}
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
                    </LocalToolProvider>
                  </MCPServerProvider>
                </ModelOptionsProvider>
              </SessionHistoryProvider>
            </SessionContextProvider>
          </AssistantContextProvider>
        </AssistantGroupProvider>
      </SettingsProvider>
    </div>
  );
}

export default App;
