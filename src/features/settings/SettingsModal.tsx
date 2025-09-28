import { AIServiceProvider } from '@/lib/ai-service';
import { useSettings } from '../../hooks/use-settings';
import type { ServiceConfig } from '@/context/SettingsContext';
import { useCallback, useState } from 'react';
import { dbUtils } from '@/lib/db';
import { useSessionContext } from '@/context/SessionContext';
import { useSessionHistory } from '@/context/SessionHistoryContext';
import {
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
  Modal,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  TerminalModelPicker,
} from '@/components/ui';

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export default function SettingsModal({ isOpen, onClose }: SettingsModalProps) {
  const {
    value: { serviceConfigs, windowSize },
    update,
  } = useSettings();
  const [activeTab, setActiveTab] = useState('api-key');

  const handleServiceConfigUpdate = useCallback(
    (
      provider: AIServiceProvider,
      field: keyof ServiceConfig,
      value: string,
    ) => {
      const currentConfig = serviceConfigs[provider] || {};
      update({
        serviceConfigs: {
          ...serviceConfigs,
          [provider]: { ...currentConfig, [field]: value },
        },
      });
    },
    [update, serviceConfigs],
  );

  const handleWindowSizeUpdate = useCallback(
    (size: number) => {
      update({ windowSize: size });
    },
    [update],
  );

  // Local button component for destructive full-clear flow. Kept near the
  // modal for simplicity.
  function ClearAllButton({ onClose }: { onClose: () => void }) {
    const [isClearing, setIsClearing] = useState(false);
    const { select } = useSessionContext();
    const { clearHistory } = useSessionHistory();

    return (
      <Button
        onClick={async () => {
          const ok = window.confirm(
            'Delete ALL local sessions, messages and workspace files? This cannot be undone.',
          );
          if (!ok) return;

          setIsClearing(true);
          try {
            const sessions = await dbUtils.getAllSessions();
            const errors: Array<{ id: string; err: unknown }> = [];

            for (const s of sessions) {
              try {
                await dbUtils.clearSessionAndWorkspace(s.id);
              } catch (err) {
                console.error(`Failed to clear session ${s.id}`, err);
                errors.push({ id: s.id, err });
              }
            }

            try {
              await clearHistory();
            } catch (e) {
              console.warn('clearHistory failed', e);
            }

            // Deselect current session in the SessionContext
            try {
              select(undefined);
            } catch (e) {
              console.warn('Failed to deselect session', e);
            }

            if (errors.length === 0) {
              window.alert(
                'All local sessions, messages and workspace files have been cleared.',
              );
              onClose();
            } else {
              window.alert(
                `Cleared ${sessions.length - errors.length} sessions, but ${errors.length} failed. Check console for details.`,
              );
            }
          } finally {
            setIsClearing(false);
          }
        }}
        variant="destructive"
        disabled={isClearing}
      >
        {isClearing
          ? 'Clearing...'
          : 'Clear All Sessions, Messages & Workspace'}
      </Button>
    );
  }

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="Settings"
      description="Configure your AI models, API keys, and application preferences"
      size="xl"
    >
      {/* Prevent horizontal overflow at the modal level */}
      <div className="p-6 text-gray-300 overflow-x-hidden">
        {/* Fixed height to prevent size changes when switching tabs. Add min-w-0 so flex children can shrink. */}
        <div className="min-h-[500px] max-h-[600px] overflow-y-auto flex flex-col min-w-0">
          <Tabs
            value={activeTab}
            onValueChange={setActiveTab}
            className="flex-1 flex flex-col"
          >
            {/* Allow the tabs list to scroll horizontally instead of forcing layout expansion */}
            <TabsList className="flex-shrink-0 flex gap-2 overflow-x-auto">
              <TabsTrigger value="api-key">API Key Settings</TabsTrigger>
              <TabsTrigger value="conversation-model">
                Conversation & Model Preferences
              </TabsTrigger>
              <TabsTrigger value="data-reset">Data & Reset</TabsTrigger>
            </TabsList>

            <TabsContent value="api-key" className="pt-4">
              <div className="flex flex-col gap-6 min-w-0">
                {Object.values(AIServiceProvider)
                  .filter((provider) => provider !== AIServiceProvider.Empty)
                  .map((provider) => {
                    const config = serviceConfigs[provider] || {};
                    const providerName =
                      provider.charAt(0).toUpperCase() + provider.slice(1);

                    return (
                      <Card
                        key={provider}
                        className="bg-background border shadow-sm min-w-0 w-full"
                      >
                        <CardHeader className="pb-2">
                          <CardTitle className="text-foreground text-base font-medium break-words">
                            {providerName}
                          </CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-3 min-w-0">
                          <div className="min-w-0">
                            <label className="block text-muted-foreground mb-2 text-sm font-medium">
                              API Key
                            </label>
                            <Input
                              type="password"
                              placeholder={`Enter your ${providerName} API key`}
                              value={config.apiKey || ''}
                              onChange={(e) =>
                                handleServiceConfigUpdate(
                                  provider,
                                  'apiKey',
                                  e.target.value,
                                )
                              }
                              className="bg-background border text-foreground w-full"
                            />
                          </div>
                          {provider === AIServiceProvider.Ollama && (
                            <div className="min-w-0">
                              <label className="block text-muted-foreground mb-2 text-sm font-medium">
                                Base URL
                              </label>
                              <Input
                                type="url"
                                placeholder="http://localhost:11434"
                                value={config.baseUrl || ''}
                                onChange={(e) =>
                                  handleServiceConfigUpdate(
                                    provider,
                                    'baseUrl',
                                    e.target.value,
                                  )
                                }
                                className="bg-background border text-foreground w-full"
                              />
                            </div>
                          )}
                        </CardContent>
                      </Card>
                    );
                  })}
              </div>
            </TabsContent>

            <TabsContent value="conversation-model" className="pt-4">
              <div className="space-y-6 min-w-0">
                <div className="min-w-0">
                  <label className="block text-muted-foreground mb-2 font-medium">
                    Message Window Size
                  </label>
                  <Input
                    type="number"
                    placeholder="e.g., 50"
                    value={windowSize}
                    onChange={(e) =>
                      handleWindowSizeUpdate(parseInt(e.target.value, 10))
                    }
                    className="bg-background border text-foreground w-full max-w-xs"
                  />
                </div>

                <div className="min-w-0">
                  <label className="block text-muted-foreground mb-2 font-medium">
                    LLM Preference
                  </label>
                  <TerminalModelPicker />
                </div>
              </div>
            </TabsContent>

            <TabsContent value="data-reset" className="pt-4">
              <div className="space-y-6">
                <Card className="bg-background border shadow-sm">
                  <CardHeader className="pb-2">
                    <CardTitle className="text-foreground text-base font-medium">
                      Data & Reset
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <p className="text-sm text-muted-foreground">
                      This will permanently delete all local sessions, their
                      messages, and workspace file stores from the local
                      database and native workspace directories. This action is
                      destructive and cannot be undone.
                    </p>

                    <div className="pt-4">
                      <ClearAllButton onClose={onClose} />
                    </div>
                  </CardContent>
                </Card>
              </div>
            </TabsContent>
          </Tabs>
        </div>

        <div className="flex justify-end pt-4">
          <Button onClick={onClose}>Save</Button>
        </div>
      </div>
    </Modal>
  );
}
