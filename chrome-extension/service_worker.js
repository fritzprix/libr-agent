/**
 * LibrAgent Browser Bridge — MV3 service worker (Load unpacked MVP).
 *
 * Connects to the local LibrAgent WebSocket bridge and maps sessionId → tabId
 * for createSession / navigate / closeSession / getState.
 *
 * Defaults (override via Options or chrome.storage.local):
 *   bridgePort  = 3847
 *   bridgeToken = libragent-dev
 */

const DEFAULT_PORT = 3847;
const DEFAULT_TOKEN = 'libragent-dev';
const RECONNECT_MS = 2000;
/** Keep the MV3 service worker alive so the bridge WebSocket is not dropped. */
const KEEPALIVE_ALARM = 'libragent-bridge-keepalive';
const KEEPALIVE_MINUTES = 0.4; // ~24s (Chrome minimum alarm period is ~0.5–1 min in practice)

/** @type {Map<string, number>} */
const sessionToTab = new Map();

/** @type {WebSocket | null} */
let socket = null;
/** @type {ReturnType<typeof setTimeout> | null} */
let reconnectTimer = null;
let intentionalClose = false;

/**
 * @returns {Promise<{ bridgePort: number, bridgeToken: string }>}
 */
async function loadConfig() {
  const stored = await chrome.storage.local.get(['bridgePort', 'bridgeToken']);
  const portRaw = stored.bridgePort;
  const port =
    typeof portRaw === 'number' && Number.isFinite(portRaw)
      ? portRaw
      : typeof portRaw === 'string' && portRaw.trim()
        ? Number.parseInt(portRaw, 10)
        : DEFAULT_PORT;
  const token =
    typeof stored.bridgeToken === 'string' && stored.bridgeToken.trim()
      ? stored.bridgeToken.trim()
      : DEFAULT_TOKEN;
  return {
    bridgePort: Number.isFinite(port) && port > 0 ? port : DEFAULT_PORT,
    bridgeToken: token,
  };
}

/**
 * @param {number} tabId
 * @returns {Promise<{ url: string, title: string | null, tabId: number }>}
 */
async function tabState(tabId) {
  const tab = await chrome.tabs.get(tabId);
  return {
    url: tab.url ?? 'about:blank',
    title: tab.title ?? null,
    tabId,
  };
}

/**
 * @param {{ id?: string, method?: string, params?: Record<string, unknown> }} message
 */
async function handleRequest(message) {
  const id = typeof message.id === 'string' ? message.id : null;
  const method = typeof message.method === 'string' ? message.method : '';
  const params =
    message.params && typeof message.params === 'object' ? message.params : {};

  const reply = (payload) => {
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      return;
    }
    socket.send(JSON.stringify({ id, ...payload }));
  };

  try {
    switch (method) {
      case 'createSession': {
        const sessionId = String(params.sessionId ?? '');
        const url = String(params.url ?? '');
        if (!sessionId || !url) {
          throw new Error('createSession requires sessionId and url');
        }
        const tab = await chrome.tabs.create({ url, active: true });
        if (typeof tab.id !== 'number') {
          throw new Error('chrome.tabs.create did not return a tab id');
        }
        sessionToTab.set(sessionId, tab.id);
        reply({ ok: true, result: await tabState(tab.id) });
        break;
      }
      case 'navigate': {
        const sessionId = String(params.sessionId ?? '');
        const url = String(params.url ?? '');
        const tabId = sessionToTab.get(sessionId);
        if (typeof tabId !== 'number') {
          throw new Error(`Unknown sessionId: ${sessionId}`);
        }
        if (!url) {
          throw new Error('navigate requires url');
        }
        await chrome.tabs.update(tabId, { url, active: true });
        reply({ ok: true, result: await tabState(tabId) });
        break;
      }
      case 'closeSession': {
        const sessionId = String(params.sessionId ?? '');
        const tabId = sessionToTab.get(sessionId);
        if (typeof tabId === 'number') {
          try {
            await chrome.tabs.remove(tabId);
          } catch {
            // Tab may already be closed by the user.
          }
          sessionToTab.delete(sessionId);
        }
        reply({ ok: true, result: { closed: true } });
        break;
      }
      case 'getState': {
        const sessionId = String(params.sessionId ?? '');
        const tabId = sessionToTab.get(sessionId);
        if (typeof tabId !== 'number') {
          throw new Error(`Unknown sessionId: ${sessionId}`);
        }
        reply({ ok: true, result: await tabState(tabId) });
        break;
      }
      case 'ping': {
        reply({ ok: true, result: { pong: true } });
        break;
      }
      default:
        throw new Error(`Unsupported method: ${method || '(missing)'}`);
    }
  } catch (error) {
    const errorMessage =
      error instanceof Error ? error.message : String(error ?? 'unknown error');
    reply({ ok: false, error: errorMessage });
  }
}

function scheduleReconnect() {
  if (reconnectTimer !== null) {
    return;
  }
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    void connect();
  }, RECONNECT_MS);
}

async function connect() {
  if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
    return;
  }

  const { bridgePort, bridgeToken } = await loadConfig();
  const url = `ws://127.0.0.1:${bridgePort}/extension-bridge?token=${encodeURIComponent(bridgeToken)}`;

  intentionalClose = false;
  try {
    socket = new WebSocket(url);
  } catch (error) {
    console.warn('[LibrAgent Bridge] WebSocket construct failed', error);
    scheduleReconnect();
    return;
  }

  socket.addEventListener('open', () => {
    console.info(`[LibrAgent Bridge] Connected to ${url}`);
  });

  socket.addEventListener('message', (event) => {
    let parsed;
    try {
      parsed = JSON.parse(String(event.data));
    } catch (error) {
      console.warn('[LibrAgent Bridge] Invalid JSON from bridge', error);
      return;
    }
    void handleRequest(parsed);
  });

  socket.addEventListener('close', () => {
    socket = null;
    if (!intentionalClose) {
      console.info('[LibrAgent Bridge] Disconnected; reconnecting…');
      scheduleReconnect();
    }
  });

  socket.addEventListener('error', () => {
    // close handler schedules reconnect
  });
}

function ensureKeepaliveAlarm() {
  void chrome.alarms.create(KEEPALIVE_ALARM, {
    periodInMinutes: KEEPALIVE_MINUTES,
  });
}

chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name !== KEEPALIVE_ALARM) {
    return;
  }
  // Touch the worker + re-open WS if Chrome suspended us.
  void connect();
});

chrome.runtime.onInstalled.addListener(() => {
  ensureKeepaliveAlarm();
  void connect();
});

chrome.runtime.onStartup.addListener(() => {
  ensureKeepaliveAlarm();
  void connect();
});

chrome.action.onClicked.addListener(() => {
  ensureKeepaliveAlarm();
  void connect();
});

chrome.storage.onChanged.addListener((changes, area) => {
  if (area !== 'local') {
    return;
  }
  if (changes.bridgePort || changes.bridgeToken) {
    intentionalClose = true;
    if (socket) {
      socket.close();
      socket = null;
    }
    intentionalClose = false;
    void connect();
  }
});

chrome.tabs.onRemoved.addListener((tabId) => {
  for (const [sessionId, mapped] of sessionToTab.entries()) {
    if (mapped === tabId) {
      sessionToTab.delete(sessionId);
    }
  }
});

ensureKeepaliveAlarm();
void connect();
