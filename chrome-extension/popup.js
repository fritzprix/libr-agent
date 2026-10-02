const stateEl = document.getElementById('state');
const detailEl = document.getElementById('detail');
const reconnectBtn = document.getElementById('reconnect');
const optionsLink = document.getElementById('optionsLink');

/**
 * @param {'connected' | 'reconnecting' | 'app_offline' | 'unknown'} state
 * @param {string} [detail]
 */
function render(state, detail) {
  stateEl.classList.remove('ok');
  if (state === 'connected') {
    stateEl.textContent = 'Connected';
    stateEl.classList.add('ok');
    detailEl.textContent =
      detail ||
      'LibrAgent can drive tabs in this Chrome. App restarts should reconnect on their own.';
  } else if (state === 'reconnecting') {
    stateEl.textContent = 'Reconnecting…';
    detailEl.textContent =
      detail ||
      'Looking for LibrAgent on this machine. Keep this extension enabled — you usually do not need chrome://extensions.';
  } else if (state === 'app_offline') {
    stateEl.textContent = 'LibrAgent not running';
    detailEl.textContent =
      detail ||
      'Start the LibrAgent app. This extension will reconnect automatically when the bridge is back.';
  } else {
    stateEl.textContent = 'Checking…';
    detailEl.textContent = detail || '';
  }
}

function refreshFromStorage() {
  chrome.storage.local.get(['bridgeUiState', 'bridgeUiDetail'], (stored) => {
    const state =
      stored.bridgeUiState === 'connected' ||
      stored.bridgeUiState === 'reconnecting' ||
      stored.bridgeUiState === 'app_offline'
        ? stored.bridgeUiState
        : 'unknown';
    render(state, typeof stored.bridgeUiDetail === 'string' ? stored.bridgeUiDetail : undefined);
  });
}

function askStatus() {
  chrome.runtime.sendMessage({ type: 'getBridgeStatus' }, (response) => {
    if (chrome.runtime.lastError) {
      render('unknown', chrome.runtime.lastError.message);
      return;
    }
    if (response && typeof response.state === 'string') {
      render(response.state, response.detail);
    } else {
      refreshFromStorage();
    }
  });
}

reconnectBtn.addEventListener('click', () => {
  render('reconnecting', 'Trying to reach LibrAgent…');
  chrome.runtime.sendMessage({ type: 'forceReconnect' }, () => {
    window.setTimeout(askStatus, 400);
  });
});

optionsLink.addEventListener('click', (event) => {
  event.preventDefault();
  chrome.runtime.openOptionsPage();
});

refreshFromStorage();
askStatus();
// Poll storage only — do not call connect() every tick (that raced dual sockets).
window.setInterval(refreshFromStorage, 2000);
