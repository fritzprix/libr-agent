import * as React from 'react';

const MOBILE_BREAKPOINT = 768;
const getServerSnapshot = () => false;
const mediaQueryStoreCache = new Map<
  string,
  {
    subscribe: (callback: () => void) => () => void;
    getSnapshot: () => boolean;
  }
>();

function getMediaQueryStore(query: string) {
  const cachedStore = mediaQueryStoreCache.get(query);
  if (cachedStore) {
    return cachedStore;
  }

  const store = {
    subscribe: (callback: () => void) => {
      const mql = window.matchMedia(query);
      if (mql.addEventListener) {
        mql.addEventListener('change', callback);
        return () => mql.removeEventListener('change', callback);
      }

      // Fallback for older browsers (e.g. Safari 13)
      mql.addListener(callback);
      return () => mql.removeListener(callback);
    },
    getSnapshot: () => window.matchMedia(query).matches,
  };

  mediaQueryStoreCache.set(query, store);
  return store;
}

export function useIsMobile(breakpoint = MOBILE_BREAKPOINT) {
  const query = React.useMemo(
    () => `(max-width: ${breakpoint - 1}px)`,
    [breakpoint],
  );
  const mediaQueryStore = React.useMemo(
    () => getMediaQueryStore(query),
    [query],
  );

  const isMobile = React.useSyncExternalStore(
    mediaQueryStore.subscribe,
    mediaQueryStore.getSnapshot,
    getServerSnapshot,
  );
  return !!isMobile;
}
