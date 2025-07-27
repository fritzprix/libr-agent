import { useState } from 'react';
import { Routes, Route } from 'react-router-dom';
import AppSidebar from './components/AppSidebar';
import ChatContainer from './components/ChatContainer';
import AssistantDetailList from './components/assistant/AssistantDetailList';
import AssistantGroupDetailList from './components/assistant/AssistantGroupDetailList';
import StartGroupChatView from './components/StartGroupChatView';

import History from './components/History';
import SettingsModal from './components/SettingsModal';
import { SidebarProvider } from './components/ui/sidebar';
import { WeatherTool } from './components/WeatherTool';
import { AssistantContextProvider } from './context/AssistantContext';
import { AssistantGroupProvider } from './context/AssistantGroupContext';
import { LocalToolProvider } from './context/LocalToolContext';
import { MCPServerProvider } from './context/MCPServerContext';
import { ModelOptionsProvider } from './context/ModelProvider';
import { SessionContextProvider } from './context/SessionContext';
import { SessionHistoryProvider } from './context/SessionHistoryContext';
import { SettingsProvider } from './context/SettingsContext';
import './globals.css';
import { AppHeader } from './components/AppHeader';
import { ThemeToggle } from './components/ThemeToggle';

function App() {
  const [isSettingsModalOpen, setIsSettingsModalOpen] = useState(false);
  const [showAssistantGroupManager, setShowAssistantGroupManager] =
    useState(false);

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
                                <Route path="/" element={<ChatContainer />} />
                                <Route
                                  path="/chat/single"
                                  element={<ChatContainer />}
                                />
                                <Route
                                  path="/chat/group"
                                  element={
                                    <StartGroupChatView
                                      setShowAssistantGroupManager={
                                        setShowAssistantGroupManager
                                      }
                                      showAssistantGroupManager={
                                        showAssistantGroupManager
                                      }
                                    />
                                  }
                                />
                                <Route
                                  path="/chat/flow"
                                  element={<ChatContainer />}
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
