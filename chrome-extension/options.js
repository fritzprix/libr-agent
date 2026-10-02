const DEFAULT_PORT = 3847;
const DEFAULT_TOKEN = 'libragent-dev';

const portInput = document.getElementById('bridgePort');
const tokenInput = document.getElementById('bridgeToken');
const statusEl = document.getElementById('status');
const saveBtn = document.getElementById('save');

chrome.storage.local.get(['bridgePort', 'bridgeToken'], (stored) => {
  portInput.value =
    typeof stored.bridgePort === 'number'
      ? String(stored.bridgePort)
      : typeof stored.bridgePort === 'string' && stored.bridgePort
        ? stored.bridgePort
        : String(DEFAULT_PORT);
  tokenInput.value =
    typeof stored.bridgeToken === 'string' && stored.bridgeToken
      ? stored.bridgeToken
      : DEFAULT_TOKEN;
});

saveBtn.addEventListener('click', () => {
  const port = Number.parseInt(portInput.value, 10);
  const token = tokenInput.value.trim() || DEFAULT_TOKEN;
  if (!Number.isFinite(port) || port < 1 || port > 65535) {
    statusEl.textContent = 'Port must be between 1 and 65535.';
    return;
  }
  chrome.storage.local.set({ bridgePort: port, bridgeToken: token }, () => {
    statusEl.textContent = 'Saved. Service worker will reconnect.';
  });
});
