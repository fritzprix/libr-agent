import { useState } from "react";
import { Routes, Route } from "react-router-dom";
import AppSidebar from "./components/AppSidebar";
import ChatContainer from "./components/ChatContainer";
import Group from "./components/Group";
import GroupCreationModal from "./components/GroupCreationModal";
import History from "./components/History";
import SettingsModal from "./components/SettingsModal";
import { SidebarProvider } from "./components/ui/sidebar";
import { WeatherTool } from "./components/WeatherTool";
import { AssistantContextProvider } from "./context/AssistantContext";
import { LocalToolProvider } from "./context/LocalToolContext";
import { MCPServerProvider } from "./context/MCPServerContext";
import { ModelOptionsProvider } from "./context/ModelProvider";
import { SessionContextProvider } from "./context/SessionContext";
import { SessionHistoryProvider } from "./context/SessionHistoryContext";
import { SettingsProvider } from "./context/SettingsContext";
import "./globals.css";
import { AppHeader } from "./components/AppHeader";
import { ThemeToggle } from "./components/ThemeToggle";

function App() {
  const [isSettingsModalOpen, setIsSettingsModalOpen] = useState(false);
  const [isGroupCreationModalOpen, setIsGroupCreationModalOpen] =
    useState(false);

  return (
    <div className="h-screen w-full">
      <SettingsProvider>
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
                            onOpenGroupCreationModal={() =>
                              setIsGroupCreationModalOpen(true)
                            }
                          />

                          {/* Main Content Area */}
                          <div className="flex flex-1 flex-col min-w-0">
                            <AppHeader>
                              <ThemeToggle/>
                            </AppHeader>
                            <div className="flex-1 overflow-auto w-full">
                              <Routes>
                                <Route path="/" element={<ChatContainer />} />
                                <Route path="/chat" element={<ChatContainer />} />
                                <Route path="/group" element={<Group />} />
                                <Route path="/history" element={<History />} />
                              </Routes>
                            </div>
                          </div>
                        </div>
                      </SidebarProvider>
                    <SettingsModal
                      isOpen={isSettingsModalOpen}
                      onClose={() => setIsSettingsModalOpen(false)}
                    />
                    <GroupCreationModal
                      isOpen={isGroupCreationModalOpen}
                      onClose={() => setIsGroupCreationModalOpen(false)}
                    />
                  </LocalToolProvider>
                </MCPServerProvider>
              </ModelOptionsProvider>
            </SessionHistoryProvider>
          </SessionContextProvider>
        </AssistantContextProvider>
      </SettingsProvider>
    </div>
  );
}

export default App;
