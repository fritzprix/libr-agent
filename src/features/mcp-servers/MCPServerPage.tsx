import { MCPServerManagement } from './MCPServerManagement';
import { useTranslation } from 'react-i18next';
import { Blocks } from 'lucide-react';

export default function MCPServerPage() {
  const { t } = useTranslation('common');

  return (
    <div className="p-6 h-full flex flex-col">
      <div className="flex items-center gap-3 mb-6">
        <Blocks size={32} className="text-primary" />
        <div>
          <h1 className="text-2xl text-foreground font-semibold">
            {t('settings.tabs.extensions', 'Extensions')}
          </h1>
          <p className="text-xs text-muted-foreground">
            Manage your AI extensions and tools
          </p>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto">
        <div className="max-w-5xl">
          <MCPServerManagement />
        </div>
      </div>
    </div>
  );
}
