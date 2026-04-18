import { useMemo } from 'react';
import { useSearchParams } from 'react-router-dom';
import { MCPServerManagement } from './MCPServerManagement';
import { useTranslation } from 'react-i18next';
import { Blocks } from 'lucide-react';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui';
import { SkillsManagementPanel } from '@/features/skills/SkillsManagementPanel';

const EXTENSIONS_VIEW_VALUES = ['tools', 'skills'] as const;

type ExtensionsViewValue = (typeof EXTENSIONS_VIEW_VALUES)[number];

function isExtensionsViewValue(value: string): value is ExtensionsViewValue {
  return EXTENSIONS_VIEW_VALUES.includes(value as ExtensionsViewValue);
}

export default function MCPServerPage() {
  const { t } = useTranslation('common');
  const [searchParams, setSearchParams] = useSearchParams();

  const activeView = useMemo<ExtensionsViewValue>(() => {
    const viewParam = searchParams.get('view');
    return viewParam && isExtensionsViewValue(viewParam) ? viewParam : 'tools';
  }, [searchParams]);

  const handleViewChange = (value: string) => {
    if (!isExtensionsViewValue(value)) {
      return;
    }

    const nextSearchParams = new URLSearchParams(searchParams);
    if (value === 'tools') {
      nextSearchParams.delete('view');
    } else {
      nextSearchParams.set('view', value);
    }

    setSearchParams(nextSearchParams, { replace: true });
  };

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
              {t('mcpServer.pageSubtitle', 'Manage your tools and skills')}
            </p>
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 min-h-0 overflow-y-auto pr-2 pb-4">
          <Tabs
            value={activeView}
            onValueChange={handleViewChange}
            className="space-y-6"
          >
            <TabsList className="grid w-full max-w-md grid-cols-2">
              <TabsTrigger value="tools">
                {t('mcpServer.toolsView', 'Tools')}
              </TabsTrigger>
              <TabsTrigger value="skills">
                {t('mcpServer.skillsView', 'Skills')}
              </TabsTrigger>
            </TabsList>

            <TabsContent value="tools" className="mt-0">
              <MCPServerManagement />
            </TabsContent>

            <TabsContent value="skills" className="mt-0">
              <SkillsManagementPanel />
            </TabsContent>
          </Tabs>
        </div>
      </div>
    </div>
  );
}
