import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';
import { getLogger } from '@/lib/logger';

// Static resources bundled with the app
import enCommon from '@/locales/en/common.json';
import koCommon from '@/locales/ko/common.json';
import zhCommon from '@/locales/zh/common.json';
import jaCommon from '@/locales/ja/common.json';
import frCommon from '@/locales/fr/common.json';
import esCommon from '@/locales/es/common.json';
import deCommon from '@/locales/de/common.json';
import ptCommon from '@/locales/pt/common.json';

// Initialize i18next only once
// We avoid Suspense to keep integration simple; can enable later if we switch to async backends
if (!i18n.isInitialized) {
  const logger = getLogger('i18n');
  i18n
    .use(LanguageDetector)
    .use(initReactI18next)
    .init({
      resources: {
        en: { common: enCommon },
        ko: { common: koCommon },
        zh: { common: zhCommon },
        ja: { common: jaCommon },
        fr: { common: frCommon },
        es: { common: esCommon },
        de: { common: deCommon },
        pt: { common: ptCommon },
      },
      fallbackLng: 'en',
      supportedLngs: ['en', 'ko', 'zh', 'ja', 'fr', 'es', 'de', 'pt'],
      ns: ['common'],
      defaultNS: 'common',
      interpolation: {
        escapeValue: false, // React already escapes
      },
      detection: {
        // Prefer explicit selection (localStorage), then browser
        order: ['localStorage', 'navigator'],
        caches: ['localStorage'],
      },
      react: {
        useSuspense: false,
      },
    })
    .catch((err: unknown) => {
      logger.error('i18n initialization failed', err);
    });
}

export default i18n;
