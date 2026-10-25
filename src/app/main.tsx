import { attachConsole } from '@tauri-apps/plugin-log';
import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
// i18n side-effect initialization (must come before components using useTranslation)
import '@/lib/i18n';
import { BrowserRouter } from 'react-router-dom';
import { ThemeProvider } from 'next-themes';
import { logUtils } from '@/lib/logger';
import { markStartupMilestone } from '@/lib/performance/startup-metrics';
import ErrorBoundary from '@/components/common/ErrorBoundary';
import { SettingsProvider } from '@/context/SettingsContext';
import { applyUiScale, readCachedUiScale } from '@/lib/ui-scale';

markStartupMilestone('bootstrap-script-start');

// Initialize Tauri logger
attachConsole().catch(console.error);

// Initialize global logger with default settings
// 설정은 localStorage에서 자동으로 로드되고, 없으면 기본값 사용
logUtils.initialize().catch(console.error);

// Zoom with the last applied scale before first render so the UI does not flash small-then-large.
applyUiScale(readCachedUiScale()).catch(console.error);

markStartupMilestone('root-render-start');

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <BrowserRouter>
      <ThemeProvider attribute="class" defaultTheme="dark">
        <SettingsProvider>
          <ErrorBoundary>
            <App />
          </ErrorBoundary>
        </SettingsProvider>
      </ThemeProvider>
    </BrowserRouter>
  </React.StrictMode>,
);
