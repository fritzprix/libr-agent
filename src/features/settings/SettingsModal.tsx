import { AIServiceProvider } from '@/lib/ai-service';
import { useSettings } from '../../hooks/use-settings';
import { ChangeEvent, useCallback, useState } from 'react';
import {
  Button,
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
    value: { apiKeys, windowSize },
    update,
  } = useSettings();
  const [activeTab, setActiveTab] = useState('api-key');

  const handleApiKeyUpdate = useCallback(
    (e: ChangeEvent<HTMLInputElement>, serviceProvider: AIServiceProvider) => {
      update({ apiKeys: { ...apiKeys, [serviceProvider]: e.target.value } });
    },
    [update],
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
        <Tabs value={activeTab} onValueChange={setActiveTab}>
          <TabsList>
            <TabsTrigger value="api-key">API Key Settings</TabsTrigger>
            <TabsTrigger value="conversation-model">
              Conversation & Model Preferences
            </TabsTrigger>
          </TabsList>

          <TabsContent value="api-key">
            <div className="space-y-6 pt-4">
              {Object.values(AIServiceProvider).map((serviceProvider) => (
                <div key={serviceProvider}>
                  <label className="block text-gray-400 mb-2 font-medium">
                    {serviceProvider.charAt(0).toUpperCase() +
                      serviceProvider.slice(1)}{' '}
                    API Key
                  </label>
                  <Input
                    type="password"
                    placeholder={`Enter your ${serviceProvider} API key`}
                    value={apiKeys[serviceProvider] || ''}
                    onChange={(e) => handleApiKeyUpdate(e, serviceProvider)}
                  />
                </div>
              ))}
            </div>
          </TabsContent>

          <TabsContent value="conversation-model">
            <div className="space-y-6 pt-4">
              <div>
                <label className="block text-gray-400 mb-2 font-medium">
                  Message Window Size
                </label>
                <Input
                  type="number"
                  placeholder="e.g., 50"
                  value={windowSize}
                  onChange={(e) =>
                    handleWindowSizeUpdate(parseInt(e.target.value, 10))
                  }
                />
              </div>

              <div>
                <label className="block text-gray-400 mb-2 font-medium">
                  LLM Preference
                </label>
                <TerminalModelPicker />
              </div>
            </div>
          </TabsContent>
        </Tabs>

        <div className="flex justify-end pt-4">
          <Button onClick={onClose}>Save</Button>
        </div>
      </div>
    </Modal>
  );
}
