import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { listen } from '@tauri-apps/api/event';
import { useMCPServerRegistry } from '@/context/MCPServerRegistryContext';
import { getLogger } from '@/lib/logger';
import { installPresetByName } from './installPresetByName';
import { setPresetHighlight } from './presetHighlightStore';
import { isAppControlPayload } from './types';

const logger = getLogger('AppControlBridge');
const EVENT_NAME = 'libragent:app-control';

/**
 * Listens for Rust `libragent:app-control` events and drives navigation /
 * preset highlight / one-click install / session focus for remote UI control.
 *
 * Must mount under BrowserRouter + MCPServerRegistryProvider.
 */
export function AppControlBridge(): null {
  const navigate = useNavigate();
  const { saveServer, refreshAll } = useMCPServerRegistry();

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void (async () => {
      try {
        unlisten = await listen<unknown>(EVENT_NAME, (event) => {
          void (async () => {
            if (cancelled) {
              return;
            }
            const payload = event.payload;
            if (!isAppControlPayload(payload)) {
              logger.warn('Ignoring invalid app-control payload', payload);
              return;
            }

            try {
              switch (payload.op) {
                case 'navigate': {
                  setPresetHighlight(null);
                  navigate(payload.path);
                  break;
                }
                case 'focus_session': {
                  setPresetHighlight(null);
                  navigate(`/agent/${payload.sessionId}`);
                  break;
                }
                case 'highlight': {
                  navigate('/mcp-servers');
                  setPresetHighlight(payload.name, payload.ms ?? 2500);
                  break;
                }
                case 'install_preset': {
                  navigate('/mcp-servers');
                  // refreshAll returns service data directly (not React state).
                  const installed = await refreshAll();
                  await installPresetByName(
                    payload.name,
                    saveServer,
                    installed,
                  );
                  setPresetHighlight(payload.name, 1500);
                  break;
                }
                default: {
                  const _exhaustive: never = payload;
                  return _exhaustive;
                }
              }
            } catch (error) {
              logger.error('App control handler failed', error);
            }
          })();
        });
      } catch (error) {
        logger.warn('Failed to subscribe to app-control events', error);
      }
    })();

    return () => {
      cancelled = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, [navigate, saveServer, refreshAll]);

  return null;
}
