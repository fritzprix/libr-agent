import type {
  AdvancedSettings,
  SystemSettings,
} from '@/context/SettingsContext';

export interface AdvancedSettingsSectionProps {
  localAdvancedSettings: AdvancedSettings;
  onChange: <K extends keyof AdvancedSettings>(
    key: K,
    value: AdvancedSettings[K],
  ) => void;
}

export interface AdvancedSystemSettingsProps {
  localSystemSettings: SystemSettings;
  networkSettingsChanged: boolean;
  onChange: <K extends keyof SystemSettings>(
    key: K,
    value: SystemSettings[K],
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
