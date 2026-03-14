import * as React from 'react';

const MOBILE_BREAKPOINT = 768;
const query = `(max-width: ${MOBILE_BREAKPOINT - 1}px)`;

const subscribe = (callback: () => void) => {
  const mql = window.matchMedia(query);
  if (mql.addEventListener) {
    mql.addEventListener('change', callback);
    return () => mql.removeEventListener('change', callback);
  } else {
    // Fallback for older browsers (e.g. Safari 13)
    mql.addListener(callback);
    return () => mql.removeListener(callback);
  }
};

const getSnapshot = () => window.matchMedia(query).matches;
const getServerSnapshot = () => false;

export function useIsMobile() {
  const isMobile = React.useSyncExternalStore(
    subscribe,
    getSnapshot,
    getServerSnapshot,
  );
  return !!isMobile;
}
