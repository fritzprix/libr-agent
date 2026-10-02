import type { Mermaid } from 'mermaid';

type MermaidTheme = 'dark' | 'default';

let loadPromise: Promise<Mermaid> | null = null;
let appliedTheme: MermaidTheme | null = null;
/** Serialize renders — mermaid uses a shared global config/DOM. */
let renderQueue: Promise<unknown> = Promise.resolve();

async function getMermaid(): Promise<Mermaid> {
  if (!loadPromise) {
    // Resolve via package name (vite aliases to mermaid.esm.min.mjs). Do not use
    // mermaid.core — wasm/TLA Vite plugins break its d3 dependency graph.
    loadPromise = import('mermaid').then((mod) => {
      if (!mod.default) {
        throw new Error('mermaid package is missing a default export');
      }
      return mod.default;
    });
  }
  return loadPromise;
}

function ensureInitialized(mermaid: Mermaid, theme: MermaidTheme): void {
  if (appliedTheme === theme) {
    return;
  }
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: 'strict',
    theme,
  });
  appliedTheme = theme;
}

/**
 * Lazy-load mermaid and render diagram source to SVG.
 * Theme changes re-init once; concurrent calls are queued.
 *
 * Bundling: Vite aliases `mermaid` → `mermaid.esm.min.mjs` so
 * vite-plugin-wasm / top-level-await do not break d3-color
 * (`Cannot set properties of undefined (setting 'prototype')`).
 */
export function renderMermaidSvg(
  code: string,
  isDark: boolean,
): Promise<string> {
  const theme: MermaidTheme = isDark ? 'dark' : 'default';
  const trimmed = code.trim();

  const run = async (): Promise<string> => {
    const mermaid = await getMermaid();
    ensureInitialized(mermaid, theme);
    // Mermaid requires a valid CSS id (no leading digit issues with uuid alone).
    const id = `mmd-${crypto.randomUUID().replace(/-/g, '')}`;
    try {
      const { svg } = await mermaid.render(id, trimmed);
      return svg;
    } finally {
      // mermaid.render() mounts temp nodes on document.body (svg#{id}, #d{id},
      // and optionally #i{id}). Success path usually removes them; parse/draw
      // failures can leave leftovers — clean all three.
      document.getElementById(id)?.remove();
      document.getElementById(`d${id}`)?.remove();
      document.getElementById(`i${id}`)?.remove();
    }
  };

  // Keep the queue head fulfilled so a rejected render does not stall later
  // callers. Callers still observe the real error via the returned `next`.
  const next = renderQueue.then(run, run);
  renderQueue = next.then(
    () => undefined,
    () => undefined, // swallow — next render restarts from queue head
  );
  return next;
}

/** Test-only: reset module singletons between cases. */
export function resetMermaidLoaderForTests(): void {
  loadPromise = null;
  appliedTheme = null;
  renderQueue = Promise.resolve();
}
