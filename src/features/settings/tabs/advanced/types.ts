import type {
  AdvancedSettings,
  SystemSettings,
} from '@/context/SettingsContext';

export interface AdvancedSettingsSectionProps {
  localAdvancedSettings: AdvancedSettings;
  onChange: (key: keyof AdvancedSettings, value: number) => void;
}

export interface AdvancedSystemSettingsProps {
  localSystemSettings: SystemSettings;
  networkSettingsChanged: boolean;
  onChange: (
    key: keyof SystemSettings,
    value: number | string | boolean,
  ) => void;
}

export interface DangerZoneProps {
  isDeleting: boolean;
  isResetting: boolean;
  onDelete: () => Promise<void>;
  onReset: () => Promise<void>;
}

export interface AdvancedTabProps extends AdvancedSettingsSectionProps {
  systemSettingsProps: AdvancedSystemSettingsProps;
  dangerZoneProps: DangerZoneProps;
}
