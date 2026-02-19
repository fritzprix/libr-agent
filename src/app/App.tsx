import { Route, Routes, Navigate } from 'react-router-dom';
import { lazy, Suspense, useEffect, useRef } from 'react';
import AppSidebar from '../components/layout/AppSidebar';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';

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

import { Toaster } from 'sonner';
import { ThemeToggle } from '../components/common/ThemeToggle';
import { AppHeader } from '../components/layout/AppHeader';
import { SidebarProvider } from '../components/ui/sidebar';
import { AssistantContextProvider } from '../context/AssistantContext';
import { MCPServerProvider } from '../context/MCPServerContext';
import { MCPServerRegistryProvider } from '../context/MCPServerRegistryContext';
import { ModelOptionsProvider } from '../context/ModelProvider';
import { SettingsProvider } from '../context/SettingsContext';
import { SkillsProvider } from '../context/SkillsContext';
import '../styles/globals.css';
import './App.css';
// Removed legacy tool provider imports
import { DnDContextProvider } from '@/context/DnDContext';
import { LLMServiceProvider } from '@/context/LLMServiceContext';
import { AgentSessionListProvider } from '@/context/AgentSessionListContext';
import { GlobalEventProvider } from '@/context/GlobalEventContext';

function App() {
  const hasCheckedSkills = useRef(false);

  useEffect(() => {
    const checkGlobalSkills = async () => {
      interface SkillMetadata {
        name: string;
        path: string;
      }

      let shouldPrompt = false;
      try {
        try {
          const defaultPath = await invoke<string>(
            'get_default_skills_directory',
          );
          try {
            const result = await invoke<SkillMetadata[]>(
              'scan_skills_directory',
              { directory: defaultPath },
            );
            if (Array.isArray(result) && result.length === 0)
              shouldPrompt = true;
          } catch {
            // specific error matching might be needed, but robustly: if we can't scan, maybe we should prompt?
            // Or if directory doesn't exist.
            shouldPrompt = true;
          }
        } catch {
          console.error('Failed to check skills');
        }

        if (shouldPrompt) {
          toast('Global skills not found', {
            description: 'Would you like to download the default skill set?',
            action: {
              label: 'Download',
              onClick: () => {
                const toastId = toast.loading('Downloading global skills...');
                invoke<string>('download_global_skills')
                  .then(() => {
                    toast.success('Skills downloaded successfully', {
                      id: toastId,
                    });
                  })
                  .catch((err) => {
                    toast.error(`Download failed: ${err}`, { id: toastId });
                  });
              },
            },
            cancel: {
              label: 'Cancel',
              onClick: () => {},
            },
            duration: Infinity,
          });
        }
      } catch (error) {
        console.error('Startup check failed:', error);
      }
    };

    if (!hasCheckedSkills.current) {
      hasCheckedSkills.current = true;
      checkGlobalSkills();
    }
  }, []);

  return (
    <div className="h-screen w-full">
      <SettingsProvider>
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
      </SettingsProvider>
    </div>
  );
}

export default App;
