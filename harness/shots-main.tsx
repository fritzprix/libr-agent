import React from 'react';
import { createRoot } from 'react-dom/client';
import { MemoryRouter } from 'react-router-dom';
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import enCommon from '../../../src/locales/en/common.json';
import '../../../src/styles/globals.css';
import { ServerCard } from '../../../src/features/mcp-servers/components/ServerCard';
import { shotState } from './mockBackend';

const oauthServer = {
  id: 'srv-001',
  name: 'linear',
  isActive: true,
  transport: { type: 'http', url: 'https://mcp.linear.app/mcp' },
  createdAt: new Date(),
  updatedAt: new Date(),
  authentication: {
    type: 'oauth2.1',
    authorizationEndpoint: 'https://auth.linear.app/oauth/authorize',
    tokenEndpoint: 'https://auth.linear.app/oauth/token',
    clientId: 'libragent',
    redirectUri: 'http://localhost:14207/callback',
    usePKCE: true,
  },
  verificationStatus: 'error',
};

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function main() {
  await i18n.use(initReactI18next).init({
    lng: 'en',
    fallbackLng: 'en',
    ns: ['common'],
    defaultNS: 'common',
    resources: { en: { common: enCommon } },
    interpolation: { escapeValue: false },
  });

  const root = createRoot(document.getElementById('root')!);
  root.render(
    <MemoryRouter>
      <div id="card-wrap" className="p-4 bg-neutral-100" style={{ width: 430 }}>
        <ServerCard
          server={oauthServer as never}
          onEdit={() => {}}
          onDelete={() => {}}
          onToggleActive={() => {}}
        />
      </div>
    </MemoryRouter>,
  );

  await sleep(60);
  const authorize = document.querySelector(
    '[data-testid="oauth-authorize"]',
  ) as HTMLElement | null;
  if (authorize) {
    authorize.click();
    await sleep(80);
  }
  if (shotState === 'cancelled') {
    const cancel = document.querySelector('[data-testid="oauth-cancel"]') as
      HTMLElement | null;
    cancel?.click();
    await sleep(120);
  } else {
    await sleep(240);
  }
  (window as unknown as { __shotReady: boolean }).__shotReady = true;
}

void main();
