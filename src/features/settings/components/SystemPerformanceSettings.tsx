import { SystemAutomationSection } from './system-performance/SystemAutomationSection';
import { SystemBackgroundTasksSection } from './system-performance/SystemBackgroundTasksSection';
import { SystemNetworkSection } from './system-performance/SystemNetworkSection';
import type { SystemPerformanceSettingsProps } from './system-performance/types';

export function SystemPerformanceSettings({
  localSystemSettings,
  networkSettingsChanged,
  onChange,
}: SystemPerformanceSettingsProps) {
  return (
    <div className="space-y-8">
      <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
        <SystemBackgroundTasksSection
          localSystemSettings={localSystemSettings}
          onChange={onChange}
        />
        <SystemAutomationSection
          localSystemSettings={localSystemSettings}
          onChange={onChange}
        />
      </div>

      <SystemNetworkSection
        localSystemSettings={localSystemSettings}
        networkSettingsChanged={networkSettingsChanged}
        onChange={onChange}
      />
    </div>
  );
}
