import { useCallback } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import { useSessionContext } from '@/context/SessionContext';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useSessionNavigation');

/**
 * Custom hook for session selection with automatic navigation.
 * Provides a convenient way to select a session and navigate to chat view in one action.
 *
 * @example
 * ```tsx
 * const { selectAndNavigate } = useSessionNavigation();
 * // Select session and auto-navigate to chat
 * selectAndNavigate(sessionId);
 * ```
 */
export function useSessionNavigation() {
  const { current, select } = useSessionContext();
  const navigate = useNavigate();
  const location = useLocation();

  /**
   * Select a session and automatically navigate to chat view if not already there.
   * @param sessionId - The ID of the session to select, or undefined to clear selection
   */
  const selectAndNavigate = useCallback(
    (sessionId?: string) => {
      // First, update the session selection
      select(sessionId);

      // If a session is selected and user is not on chat route, navigate to chat
      if (sessionId && !location.pathname.startsWith('/chat')) {
        logger.info('Session selected, navigating to chat', {
          sessionId,
          currentPath: location.pathname,
        });
        navigate('/chat/single');
      } else if (!sessionId) {
        // If clearing selection (sessionId is undefined), no navigation needed
        logger.debug('Session selection cleared', {
          currentPath: location.pathname,
        });
      } else {
        logger.debug('Already on chat route, no navigation needed', {
          sessionId,
          currentPath: location.pathname,
        });
      }
    },
    [select, navigate, location],
  );

  return {
    /**
     * Select a session and navigate to chat (if needed)
     */
    selectAndNavigate,
    /**
     * Current selected session
     */
    current,
    /**
     * Direct access to select without navigation (for special cases)
     */
    select,
  };
}
