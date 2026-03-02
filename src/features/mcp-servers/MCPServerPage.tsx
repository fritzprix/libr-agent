import { MCPServerManagement } from './MCPServerManagement';
import { useTranslation } from 'react-i18next';
import { Blocks } from 'lucide-react';

export default function MCPServerPage() {
  const { t } = useTranslation('common');

  return (
    <div className="p-6 h-full flex flex-col bg-background">
      <div className="max-w-5xl mx-auto w-full flex flex-col h-full">
        {/* Header */}
        <div className="flex items-center gap-4 mb-8">
          <div className="flex items-center justify-center p-2.5 bg-primary/10 text-primary rounded-xl">
            <Blocks size={28} />
          </div>
          <div>
            <h1 className="text-2xl text-foreground font-semibold tracking-tight">
              {t('settings.tabs.extensions', 'Extensions')}
            </h1>
            <p className="text-sm text-muted-foreground mt-0.5">
              Manage your AI extensions and tools
            </p>
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 min-h-0 overflow-y-auto pr-2 pb-4">
          <MCPServerManagement />
        </div>
      </div>
    </div>
  );
}
