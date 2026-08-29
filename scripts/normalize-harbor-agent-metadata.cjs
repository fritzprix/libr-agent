'use strict';

const fs = require('node:fs');
const path = require('node:path');

const HERMES_AGENT_NAME = 'hermes';

/**
 * @param {unknown} value
 * @returns {value is Record<string, unknown>}
 */
function isRecord(value) {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/**
 * Remove environment-specific details from a Hermes version string.
 *
 * Harbor uses the complete version string as part of the agent identity. Hermes
 * appends its install path and Python version, so identical Hermes releases can
 * otherwise become separate agents across task containers.
 *
 * @param {unknown} value
 * @returns {string | null}
 */
function normalizeHermesVersion(value) {
  if (typeof value !== 'string') {
    return null;
  }

  const releaseLine = value.split(/\r?\n/u, 1)[0].trim();
  return releaseLine.startsWith('Hermes Agent v') ? releaseLine : null;
}

/**
 * @param {string} filePath
 * @returns {Record<string, unknown>}
 */
function readJson(filePath) {
  const payload = JSON.parse(fs.readFileSync(filePath, 'utf8'));
  if (!isRecord(payload)) {
    throw new Error(`Expected a JSON object in ${filePath}`);
  }
  return payload;
}

/**
 * @param {string} filePath
 * @param {Record<string, unknown>} payload
 */
function writeJson(filePath, payload) {
  fs.writeFileSync(filePath, `${JSON.stringify(payload, null, 2)}\n`, 'utf8');
}

/**
 * Normalize one Hermes trial's result and trajectory metadata.
 *
 * @param {string} trialDir
 * @returns {number} Number of files changed.
 */
function normalizeHermesTrial(trialDir) {
  const resultPath = path.join(trialDir, 'result.json');
  const result = readJson(resultPath);
  const agentInfo = result.agent_info;

  if (!isRecord(agentInfo) || agentInfo.name !== HERMES_AGENT_NAME) {
    return 0;
  }

  const canonicalVersion = normalizeHermesVersion(agentInfo.version);
  if (!canonicalVersion) {
    return 0;
  }

  let changedFiles = 0;
  if (agentInfo.version !== canonicalVersion) {
    agentInfo.version = canonicalVersion;
    writeJson(resultPath, result);
    changedFiles += 1;
  }

  const trajectoryPath = path.join(trialDir, 'agent', 'trajectory.json');
  if (!fs.existsSync(trajectoryPath)) {
    return changedFiles;
  }

  const trajectory = readJson(trajectoryPath);
  const trajectoryAgent = trajectory.agent;
  if (
    !isRecord(trajectoryAgent) ||
    trajectoryAgent.name !== HERMES_AGENT_NAME ||
    trajectoryAgent.version === canonicalVersion
  ) {
    return changedFiles;
  }

  trajectoryAgent.version = canonicalVersion;
  writeJson(trajectoryPath, trajectory);
  return changedFiles + 1;
}

/**
 * Normalize environment-dependent Hermes metadata in a Harbor job.
 *
 * The source job is intentionally updated in place so a later re-upload uses
 * the same stable identity and the archived trial metadata stays consistent.
 *
 * @param {string} jobDir
 * @returns {number} Number of metadata files changed.
 */
function normalizeHermesJob(jobDir) {
  const entries = fs.readdirSync(jobDir, { withFileTypes: true });
  let changedFiles = 0;

  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }

    const trialDir = path.join(jobDir, entry.name);
    if (fs.existsSync(path.join(trialDir, 'result.json'))) {
      changedFiles += normalizeHermesTrial(trialDir);
    }
  }

  return changedFiles;
}

module.exports = {
  normalizeHermesJob,
  normalizeHermesTrial,
  normalizeHermesVersion,
};
