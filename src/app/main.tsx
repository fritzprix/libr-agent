import { attachConsole } from '@tauri-apps/plugin-log';
import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import { BrowserRouter } from 'react-router';
import { ThemeProvider } from 'next-themes';
import { logUtils } from '@/lib/logger';

// Initialize Tauri logger
attachConsole().catch(console.error);

// Initialize global logger with default settings
// 설정은 localStorage에서 자동으로 로드되고, 없으면 기본값 사용
logUtils.initialize().catch(console.error);

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <BrowserRouter>
      <ThemeProvider attribute="class" defaultTheme="dark">
        <App />
      </ThemeProvider>
    </BrowserRouter>
  </React.StrictMode>,
);
