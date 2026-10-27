'use strict';
/**
 * Workspace-mode decision for run-harbor-bench.cjs: which modes to run and which one
 * to export before a non-isolated run.
 */

const WORKSPACE_MODE_FLAG = '--workspace-mode';
const WORKSPACE_MODES = ['managed', 'host'];

/**
 * `host` is the default but runs task commands on this machine, so it needs the
 * sandboxed instance (--isolated); other runs default to `managed`.
 * @param {string[]} argv Arguments without the isolated flag.
 * @param {boolean} isolated
 * @param {string | undefined} envMode LIBRAGENT_HARBOR_WORKSPACE_MODE, when preset.
 * @returns {{ argv: string[], modes: string[], immediate: string | null, error: string | null }}
 */
function planWorkspaceModes(argv, isolated, envMode) {
  const rest = [...argv];
  const fail = (/** @type {string} */ error) => ({
    argv: rest,
    modes: [],
    immediate: null,
    error,
  });
  const index = rest.indexOf(WORKSPACE_MODE_FLAG);
  const preset = envMode && envMode.trim() ? envMode.trim() : undefined;
  let requested = preset;
  let given = preset !== undefined;
  if (index !== -1) {
    requested = rest.splice(index, 2)[1];
    given = true;
  }
  if (!given) {
    requested = isolated ? 'host' : 'managed';
  } else if (
    requested === undefined ||
    (requested !== 'all' && !WORKSPACE_MODES.includes(requested))
  ) {
    const got = requested === undefined ? 'no value' : `'${requested}'`;
    return fail(
      `${WORKSPACE_MODE_FLAG} must be one of ${[...WORKSPACE_MODES, 'all'].join(', ')} (got ${got}).`,
    );
  }
  if ((requested === 'host' || requested === 'all') && !isolated) {
    return fail(
      `${WORKSPACE_MODE_FLAG} ${requested} runs task commands on this machine and requires --isolated.`,
    );
  }
  const modes = requested === 'all' ? WORKSPACE_MODES : [requested];
  return {
    argv: rest,
    modes,
    immediate: isolated ? null : requested,
    error: null,
  };
}

module.exports = { WORKSPACE_MODE_FLAG, WORKSPACE_MODES, planWorkspaceModes };
