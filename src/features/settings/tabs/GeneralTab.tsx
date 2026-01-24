import { useTranslation } from 'react-i18next';

interface GeneralTabProps {
  localLanguage: string;
  onChange: (lang: string) => void;
}

export function GeneralTab({ localLanguage, onChange }: GeneralTabProps) {
  const { t } = useTranslation('common');

  return (
    <div className="space-y-6">
      <div className="min-w-0">
        <label className="block text-muted-foreground mb-2 font-medium">
          {t('settings.language.label', 'Language')}
        </label>
        <select
          className="bg-background border text-foreground rounded px-3 py-2 w-full max-w-xs"
          value={localLanguage}
          onChange={(e) => onChange(e.target.value)}
        >
          <option value="en">
            {t('settings.language.english', 'English')}
          </option>
          <option value="ko">{t('settings.language.korean', 'Korean')}</option>
        </select>
      </div>
    </div>
  );
}
