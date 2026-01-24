import { useTranslation } from 'react-i18next';
import { SystemSettings } from '@/context/SettingsContext';
import { Input } from '@/components/ui';

interface SystemPerformanceSettingsProps {
  localSystemSettings: SystemSettings;
  onChange: (key: keyof SystemSettings, value: number) => void;
}

export function SystemPerformanceSettings({
  localSystemSettings,
  onChange,
}: SystemPerformanceSettingsProps) {
  const { t } = useTranslation('common');

  return (
    <div className="border-t pt-8 mt-4">
      <h3 className="text-lg font-medium text-foreground mb-4">
        {t('settings.system.title', 'System & Performance')}
      </h3>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-6">
        {/* File & Workspace */}
        <div className="space-y-4">
          <h4 className="text-sm font-medium text-foreground">
            {t('settings.system.fileWorkspace', 'File & Workspace')}
          </h4>
          {/* Max File Upload Size */}
          <div className="min-w-0">
            <label className="block text-muted-foreground mb-2 font-medium">
              {t(
                'settings.system.maxFileUploadSize',
                'Max File Upload Size (MB)',
              )}
            </label>
            <Input
              type="number"
              placeholder="e.g., 50"
              min={1}
              value={localSystemSettings.maxFileUploadSizeMB}
              onChange={(e) =>
                onChange(
                  'maxFileUploadSizeMB',
                  parseInt(e.target.value, 10) || 50,
                )
              }
              className="bg-background border text-foreground w-full max-w-xs"
            />
            <p className="text-xs text-muted-foreground mt-1">
              {t(
                'settings.system.maxFileUploadSizeDescription',
                'Maximum size for a single file upload. Increase if you often work with large documents.',
              )}
            </p>
          </div>

          {/* Workspace Capacity */}
          <div className="min-w-0">
            <label className="block text-muted-foreground mb-2 font-medium">
              {t(
                'settings.system.workspaceCapacity',
                'Workspace Capacity (MB)',
              )}
            </label>
            <Input
              type="number"
              placeholder="e.g., 10"
              min={1}
              value={localSystemSettings.workspaceCapacityMB}
              onChange={(e) =>
                onChange(
                  'workspaceCapacityMB',
                  parseInt(e.target.value, 10) || 10,
                )
              }
              className="bg-background border text-foreground w-full max-w-xs"
            />
            <p className="text-xs text-muted-foreground mt-1">
              {t(
                'settings.system.workspaceCapacityDescription',
                "Total limit for your current workspace's text content.",
              )}
            </p>
          </div>
        </div>

        {/* Background Tasks */}
        <div className="space-y-4">
          <h4 className="text-sm font-medium text-foreground">
            {t('settings.system.backgroundTasks', 'Background Tasks')}
          </h4>
          {/* Search Index Frequency */}
          <div className="min-w-0">
            <label className="block text-muted-foreground mb-2 font-medium">
              {t(
                'settings.system.searchIndexFrequency',
                'Search Index Frequency (Min)',
              )}
            </label>
            <Input
              type="number"
              placeholder="e.g., 5"
              min={1}
              value={localSystemSettings.searchIndexFrequencyMinutes}
              onChange={(e) =>
                onChange(
                  'searchIndexFrequencyMinutes',
                  parseInt(e.target.value, 10) || 5,
                )
              }
              className="bg-background border text-foreground w-full max-w-xs"
            />
            <p className="text-xs text-muted-foreground mt-1">
              {t(
                'settings.system.searchIndexFrequencyDescription',
                'How often the AI updates its memory search. Faster updates keep search fresh but use more battery/CPU.',
              )}
            </p>
          </div>

          {/* Web Action Timeout */}
          <div className="min-w-0">
            <label className="block text-muted-foreground mb-2 font-medium">
              {t(
                'settings.system.webActionTimeout',
                'Web Action Timeout (Sec)',
              )}
            </label>
            <Input
              type="number"
              placeholder="e.g., 30"
              min={5}
              value={localSystemSettings.webActionTimeoutSeconds}
              onChange={(e) =>
                onChange(
                  'webActionTimeoutSeconds',
                  parseInt(e.target.value, 10) || 30,
                )
              }
              className="bg-background border text-foreground w-full max-w-xs"
            />
            <p className="text-xs text-muted-foreground mt-1">
              {t(
                'settings.system.webActionTimeoutDescription',
                'How long the AI waits for a webpage to load or a click to finish.',
              )}
            </p>
          </div>

          {/* Session Retention */}
          <div className="min-w-0">
            <label className="block text-muted-foreground mb-2 font-medium">
              {t(
                'settings.system.activeSessionRetention',
                'Keep Active Sessions For (Hours)',
              )}
            </label>
            <Input
              type="number"
              placeholder="e.g., 24"
              min={1}
              value={localSystemSettings.activeSessionRetentionHours}
              onChange={(e) =>
                onChange(
                  'activeSessionRetentionHours',
                  parseInt(e.target.value, 10) || 24,
                )
              }
              className="bg-background border text-foreground w-full max-w-xs"
            />
            <p className="text-xs text-muted-foreground mt-1">
              {t(
                'settings.system.activeSessionRetentionDescription',
                'How long to keep session data in fast memory.',
              )}
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
