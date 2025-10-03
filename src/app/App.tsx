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
import { AssistantGroupProvider } from '../context/AssistantGroupContext';
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
import { ToolsTestPage } from '@/features/tools/ToolsTestPage';
import { SystemPromptProvider } from '@/context/SystemPromptContext';
import { MCPUITest } from '@/components/mcp-ui-test/MCPUITest';
import WebMCPTest from '@/test/WebMCPTest';
import { DnDContextProvider } from '@/context/DnDContext';
import DnDTestPage from '@/features/dnd/DnDTestPage';
import SettingsPage from '@/features/settings/SettingsPage';

function App() {
  return (
    <div className="h-screen w-full">
      <SettingsProvider>
        <MCPServerProvider>
          <SystemPromptProvider>
            <AssistantGroupProvider>
              <AssistantContextProvider>
                <SessionContextProvider>
                  <BuiltInToolProvider>
                    <WebMCPProvider>
                      <WebMCPServiceRegistry
                        servers={['planning', 'playbook']}
                      />
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
                                        path="/tools/test"
                                        element={<ToolsTestPage />}
                                      />
                                      <Route
                                        path="/mcp-ui/test"
                                        element={<MCPUITest />}
                                      />
                                      <Route
                                        path="/webmcp/test"
                                        element={<WebMCPTest />}
                                      />
                                      <Route
                                        path="/dnd/test"
                                        element={<DnDTestPage />}
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
            </AssistantGroupProvider>
          </SystemPromptProvider>
        </MCPServerProvider>
      </SettingsProvider>
    </div>
  );
}

export default App;
