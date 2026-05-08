import type { FC } from 'react';
import { BrainCircuit, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  Button,
  Badge,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/components/ui';
import GeneralTab from './tabs/GeneralTab';
import AIModelsTab from './tabs/AIModelsTab';
import ChatInterfaceTab from './tabs/ChatInterfaceTab';
import SystemTab from './tabs/SystemTab';
import AdvancedTab from './tabs/AdvancedTab';
import DevTab from './tabs/DevTab';
import {
  PROVIDER_ENTRIES,
  useSettingsPageController,
} from './hooks/useSettingsPageController';

const SettingsPage: FC = function SettingsPage() {
  const { t } = useTranslation('common');
  const {
    formState,
    updateDisplay,
    updateAdvanced,
    activeTab,
    changedSectionCount,
    dangerZoneProps,
    handleClose,
    handleContextStrategyChange,
    handleDefaultMaxOutputTokensChange,
    handleDiscard,
    handleDiscardAndLeave,
    handleFallbackModelChange,
    handleLanguageChange,
    handleMaxInputContextChange,
    handleMaxRetriesChange,
    handlePendingChange,
    handlePreferredModelChange,
    handleRetryDelayChange,
    handleSave,
    handleSaveAndLeave,
    handleTabChange,
    handleToolCallGroupVisibleCountChange,
    handleWindowSizeChange,
    isDirty,
    isDiscardDialogOpen,
    isLeaveDialogOpen,
    isSaving,
    networkSettingsChanged,
    setIsDiscardDialogOpen,
    setIsLeaveDialogOpen,
    systemSettingsProps,
    tabNavigationItems,
  } = useSettingsPageController();

  return (
    <div className="p-6 h-full flex flex-col bg-background">
      <div className="max-w-4xl mx-auto w-full flex flex-col h-full">
        {/* Header */}
        <div className="flex items-center justify-between mb-8">
          <div className="flex items-center gap-4">
            <div className="flex items-center justify-center p-2.5 bg-primary/10 text-primary rounded-xl">
              <BrainCircuit size={28} />
            </div>
            <div>
              <h1 className="text-2xl text-foreground font-semibold tracking-tight">
                {t('settings.title', 'Settings')}
              </h1>
              <p className="text-sm text-muted-foreground mt-0.5">
                {t('settings.versionLabel', {
                  defaultValue: '{{appName}} v{{version}}',
                  appName: t('appName', 'LibrAgent'),
                  version: __APP_VERSION__,
                })}
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2 flex-wrap justify-end">
            {isDirty && (
              <Badge
                variant="outline"
                className="border-warning/30 bg-warning/10 text-warning-foreground"
              >
                {t('settings.pendingChanges', {
                  count: changedSectionCount,
                  defaultValue: '{{count}} sections changed',
                })}
              </Badge>
            )}
            {networkSettingsChanged && (
              <Badge
                variant="outline"
                className="border-warning/30 bg-warning/10 text-warning-foreground"
              >
                {t(
                  'settings.system.restartRequired',
                  'Restart required after save',
                )}
              </Badge>
            )}
            <Button
              onClick={() => setIsDiscardDialogOpen(true)}
              variant="outline"
              className="h-9"
              disabled={!isDirty || isSaving}
            >
              {t('settings.discardChanges', 'Discard')}
            </Button>
            <Button
              onClick={handleClose}
              variant="ghost"
              className="h-9"
              disabled={isSaving}
            >
              {t('common.close', 'Close')}
            </Button>
            <Button
              onClick={handleSave}
              disabled={!isDirty || isSaving}
              className="h-9 font-medium"
            >
              {isSaving && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              {isSaving
                ? t('settings.saving', 'Saving...')
                : t('settings.saveChanges', 'Save Changes')}
            </Button>
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 min-h-0 overflow-y-auto pr-2 pb-4">
          <Tabs
            value={activeTab}
            onValueChange={handleTabChange}
            className="flex flex-col min-h-full"
          >
            <TabsList className="sticky top-0 z-10 mb-4 flex gap-2 overflow-x-auto border border-border/60 bg-background/95 p-1 backdrop-blur supports-[backdrop-filter]:bg-background/80">
              {tabNavigationItems.map((tab) => (
                <TabsTrigger
                  key={tab.value}
                  value={tab.value}
                  className={`gap-2 ${tab.className ?? ''}`.trim()}
                >
                  {tab.label}
                  {tab.isDirty && (
                    <span className="h-1.5 w-1.5 rounded-full bg-warning" />
                  )}
                </TabsTrigger>
              ))}
            </TabsList>

            <TabsContent value="general">
              <GeneralTab
                localLanguage={formState.uiLanguage}
                onChange={handleLanguageChange}
                localDisplay={formState.display}
                onDisplaySettingsChange={updateDisplay}
              />
            </TabsContent>

            <TabsContent value="ai-models">
              <AIModelsTab
                serviceConfigs={formState.serviceConfigs}
                providerEntries={PROVIDER_ENTRIES}
                localPreferredModel={formState.preferredModel}
                localFallbackModel={formState.fallbackModel}
                localMaxRetries={formState.advanced.maxRetries}
                localRetryDelay={formState.advanced.retryDelay}
                localDefaultMaxOutputTokens={
                  formState.advanced.defaultMaxOutputTokens
                }
                onPendingChange={handlePendingChange}
                onPreferredModelChange={handlePreferredModelChange}
                onFallbackModelChange={handleFallbackModelChange}
                onMaxRetriesChange={handleMaxRetriesChange}
                onRetryDelayChange={handleRetryDelayChange}
                onDefaultMaxOutputTokensChange={
                  handleDefaultMaxOutputTokensChange
                }
              />
            </TabsContent>

            <TabsContent value="chat-interface">
              <ChatInterfaceTab
                localContextStrategy={formState.contextStrategy}
                localWindowSize={formState.windowSize}
                localMaxInputContext={formState.maxInputContext}
                localToolCallGroupVisibleCount={
                  formState.toolCallGroupVisibleCount
                }
                localAdvancedSettings={formState.advanced}
                onContextStrategyChange={handleContextStrategyChange}
                onWindowSizeChange={handleWindowSizeChange}
                onMaxInputContextChange={handleMaxInputContextChange}
                onToolCallGroupVisibleCountChange={
                  handleToolCallGroupVisibleCountChange
                }
                onAdvancedSettingsChange={updateAdvanced}
              />
            </TabsContent>

            <TabsContent value="system">
              <SystemTab systemSettingsProps={systemSettingsProps} />
            </TabsContent>

            <TabsContent value="advanced">
              <AdvancedTab
                localAdvancedSettings={formState.advanced}
                onChange={updateAdvanced}
                systemSettingsProps={systemSettingsProps}
                dangerZoneProps={dangerZoneProps}
              />
            </TabsContent>

            {import.meta.env.DEV && (
              <TabsContent value="dev">
                <DevTab serviceConfigs={formState.serviceConfigs} />
              </TabsContent>
            )}
          </Tabs>
        </div>
      </div>

      <Dialog open={isDiscardDialogOpen} onOpenChange={setIsDiscardDialogOpen}>
        <DialogContent showCloseButton={!isSaving}>
          <DialogHeader>
            <DialogTitle>
              {t('settings.discardTitle', 'Discard unsaved changes?')}
            </DialogTitle>
            <DialogDescription>
              {t(
                'settings.discardDescription',
                'This will revert every pending change on this page back to the last saved state.',
              )}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setIsDiscardDialogOpen(false)}
              disabled={isSaving}
            >
              {t('common.cancel', 'Cancel')}
            </Button>
            <Button
              variant="destructive"
              onClick={handleDiscard}
              disabled={isSaving}
            >
              {t('settings.discardChanges', 'Discard')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={isLeaveDialogOpen} onOpenChange={setIsLeaveDialogOpen}>
        <DialogContent showCloseButton={!isSaving}>
          <DialogHeader>
            <DialogTitle>
              {t('settings.leaveTitle', 'Leave without saving?')}
            </DialogTitle>
            <DialogDescription>
              {t(
                'settings.leaveDescription',
                'You have unsaved changes. Save them before leaving, or discard them and leave this page.',
              )}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setIsLeaveDialogOpen(false)}
              disabled={isSaving}
            >
              {t('common.cancel', 'Cancel')}
            </Button>
            <Button
              variant="destructive"
              onClick={handleDiscardAndLeave}
              disabled={isSaving}
            >
              {t('settings.discardAndLeave', 'Discard and Leave')}
            </Button>
            <Button onClick={handleSaveAndLeave} disabled={isSaving}>
              {isSaving && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              {t('settings.saveAndLeave', 'Save and Leave')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
};

export default SettingsPage;
