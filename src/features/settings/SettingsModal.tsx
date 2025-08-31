import { AIServiceProvider } from '@/lib/ai-service';
import { useSettings } from '../../hooks/use-settings';
import type { ServiceConfig } from '@/context/SettingsContext';
import { useCallback, useState } from 'react';
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

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="Settings"
      description="Configure your AI models, API keys, and application preferences"
      size="xl"
    >
      <div className="p-6 text-gray-300">
        {/* 고정 높이로 탭 이동 시 크기 변화 방지 */}
        <div className="min-h-[500px] max-h-[600px] overflow-y-auto flex flex-col">
          <Tabs
            value={activeTab}
            onValueChange={setActiveTab}
            className="flex-1 flex flex-col"
          >
            <TabsList>
              <TabsTrigger value="api-key">API Key Settings</TabsTrigger>
              <TabsTrigger value="conversation-model">
                Conversation & Model Preferences
              </TabsTrigger>
            </TabsList>

            <TabsContent value="api-key" className="pt-4">
              <div className="flex flex-col gap-6">
                {Object.values(AIServiceProvider)
                  .filter((provider) => provider !== AIServiceProvider.Empty)
                  .map((provider) => {
                    const config = serviceConfigs[provider] || {};
                    const providerName =
                      provider.charAt(0).toUpperCase() + provider.slice(1);

                    return (
                      <Card
                        key={provider}
                        className="bg-background border shadow-sm"
                      >
                        <CardHeader className="pb-2">
                          <CardTitle className="text-foreground text-base font-medium">
                            {providerName}
                          </CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-3">
                          <div>
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
                              className="bg-background border text-foreground"
                            />
                          </div>
                          {provider === AIServiceProvider.Ollama && (
                            <div>
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
                                className="bg-background border text-foreground"
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
              <div className="space-y-6">
                <div>
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
                    className="bg-background border text-foreground max-w-xs"
                  />
                </div>

                <div>
                  <label className="block text-muted-foreground mb-2 font-medium">
                    LLM Preference
                  </label>
                  <TerminalModelPicker />
                </div>
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
