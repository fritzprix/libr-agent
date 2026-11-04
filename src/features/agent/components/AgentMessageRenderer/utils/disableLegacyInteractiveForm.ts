import type { UiResourceLike } from './injectUiResourceTheme';

/** URI prefix of the removed `ui__presentInteractive` forms in old transcripts. */
const LEGACY_INTERACTIVE_URI_PREFIX = 'ui://interactive/';

const READONLY_MARKER = 'data-libragent-readonly';

// The iframe has no same-origin access, so the controls are disabled from inside.
const READONLY_BLOCK = `<style ${READONLY_MARKER}="true">
input, select, textarea, button { pointer-events: none !important; opacity: 0.6; }
</style>
<script ${READONLY_MARKER}="true">
document.querySelectorAll('input, select, textarea, button').forEach(function (el) {
  el.disabled = true;
});
</script>`;

/**
 * Old `presentInteractive` forms still render, but nothing can answer them now,
 * so their controls are disabled. Other resources are returned unchanged.
 */
export function disableLegacyInteractiveForm<T extends UiResourceLike>(
  resource: T,
): T {
  if (
    !resource.uri?.startsWith(LEGACY_INTERACTIVE_URI_PREFIX) ||
    typeof resource.text !== 'string' ||
    resource.text.includes(READONLY_MARKER)
  ) {
    return resource;
  }

  const bodyClose = resource.text.search(/<\/body>/i);
  const text =
    bodyClose === -1
      ? resource.text + READONLY_BLOCK
      : resource.text.slice(0, bodyClose) +
        READONLY_BLOCK +
        resource.text.slice(bodyClose);
  return { ...resource, text };
}
