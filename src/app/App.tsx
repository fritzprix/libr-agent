import { Route, Routes } from 'react-router-dom';
import AppSidebar from '../components/layout/AppSidebar';

import ChatContainer from '@/features/chat/ChatContainer';
import { Toaster } from 'sonner';
import { ThemeToggle } from '../components/common/ThemeToggle';
import { AppHeader } from '../components/layout/AppHeader';
import { SidebarProvider } from '../components/ui/sidebar';
import { AssistantContextProvider } from '../context/AssistantContext';
import { MCPServerProvider } from '../context/MCPServerContext';
import { ModelOptionsProvider } from '../context/ModelProvider';
import { SessionContextProvider } from '../context/SessionContext';
import { SessionHistoryProvider } from '../context/SessionHistoryContext';
import { SettingsProvider } from '../context/SettingsContext';
import History from '../features/history/History';
import '../styles/globals.css';
import './App.css';
import AssistantList from '@/features/assistant/List';
import { ResourceAttachmentProvider } from '@/context/ResourceAttachmentContext';
import { BuiltInToolProvider } from '@/features/tools';
import { BrowserToolProvider } from '@/features/tools/BrowserToolProvider';
import { RustMCPToolProvider } from '@/features/tools/RustMCPToolProvider';
import { WebMCPServiceRegistry } from '@/features/tools/WebMCPServiceRegistry';
import { WebMCPProvider } from '@/context/WebMCPContext';
import { SystemPromptProvider } from '@/context/SystemPromptContext';
import { DnDContextProvider } from '@/context/DnDContext';
import SettingsPage from '@/features/settings/SettingsPage';
import MessageSearch from '@/features/search/MessageSearch';

function App() {
  return (
    <div className="h-screen w-full">
      <SettingsProvider>
        <MCPServerProvider>
          <SystemPromptProvider>
            <AssistantContextProvider>
              <SessionContextProvider>
                <BuiltInToolProvider>
                  <WebMCPProvider>
                    <WebMCPServiceRegistry servers={['planning', 'playbook', 'ui']} />
                    <BrowserToolProvider />
                    <RustMCPToolProvider />
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
                                <div className="flex-1 overflow-auto w-full">
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
                                      path="/search"
                                      element={<MessageSearch />}
                                    />
                                    <Route
                                      path="/settings"
                                      element={<SettingsPage />}
                                    />
                                  </Routes>
                                </div>
                              </div>
                            </DnDContextProvider>
                          </SidebarProvider>
                          <Toaster />
                        </ModelOptionsProvider>
                      </ResourceAttachmentProvider>
                    </SessionHistoryProvider>
                  </WebMCPProvider>
                </BuiltInToolProvider>
              </SessionContextProvider>
            </AssistantContextProvider>
          </SystemPromptProvider>
        </MCPServerProvider>
      </SettingsProvider>
    </div>
  );
}

export default App;
