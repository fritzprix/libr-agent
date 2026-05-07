import type { SystemSettings } from '@/context/SettingsContext';

export interface SystemPerformanceSettingsProps {
  localSystemSettings: SystemSettings;
  networkSettingsChanged: boolean;
  onChange: (key: keyof SystemSettings, value: number | boolean) => void;
}
