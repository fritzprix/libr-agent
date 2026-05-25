#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const logRootDir = path.join(repoRoot, '.refactor-logs');
const pnpmCommand = process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm';
const HEARTBEAT_INTERVAL_MS = 15_000;
const MAX_FAILURE_MATCHES = 20;
const MAX_FAILURE_TAIL_LINES = 120;
const MAX_LOG_RUN_DIRS = 10;
const FAILURE_PATTERN =
  /\b(error|errors|failed|failure|panic|panicked|fatal|segfault|exception|✗|ELIFECYCLE)\b/i;

const PREPARE_STAGES = [
  {
    name: 'sync-builtin-services',
    command: pnpmCommand,
    args: ['sync-builtin-services'],
    env: { nodeHeapMb: 512 },
  },
  {
    name: 'format',
    command: pnpmCommand,
    args: ['format'],
    env: { nodeHeapMb: 768 },
  },
  { name: 'rust:fmt', command: pnpmCommand, args: ['rust:fmt'] },
];

const VALIDATE_STAGES = [
  ...PREPARE_STAGES,
  {
    name: 'lint',
    command: pnpmCommand,
    args: ['lint'],
    env: { nodeHeapMb: 768 },
  },
  {
    name: 'format:check:all',
    command: pnpmCommand,
    args: ['format:check:all'],
    env: { nodeHeapMb: 768 },
  },
  {
    name: 'test:run:validate',
    command: pnpmCommand,
    args: ['test:run:validate'],
    env: { nodeHeapMb: 512 },
  },
  { name: 'rust:fmt:check', command: pnpmCommand, args: ['rust:fmt:check'] },
  { name: 'rust:clippy:all', command: pnpmCommand, args: ['rust:clippy:all'] },
  { name: 'rust:test', command: pnpmCommand, args: ['rust:test'] },
  {
    name: 'build:nosync',
    command: pnpmCommand,
    args: ['build:nosync'],
    env: { nodeHeapMb: 768 },
  },
  {
    name: 'perf:bundle',
    command: pnpmCommand,
    args: ['perf:bundle'],
    env: { nodeHeapMb: 512 },
  },
  {
    name: 'dead-code',
    command: pnpmCommand,
    args: ['dead-code'],
    env: { nodeHeapMb: 512 },
  },
];

export function sanitizeStageName(stageName) {
  return stageName.replace(/[^a-z0-9._-]+/gi, '-');
}

export function formatDuration(durationMs) {
  const totalSeconds = Math.max(0, Math.round(durationMs / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return minutes > 0 ? `${minutes}m ${seconds}s` : `${seconds}s`;
}

export function tailLines(text, maxLines) {
  const lines = text.split(/\r?\n/);
  const trimmedLines =
    lines.length > 0 && lines[lines.length - 1] === ''
      ? lines.slice(0, -1)
      : lines;
  return trimmedLines.slice(-maxLines);
}

export function summarizeFailureText(text) {
  const lines = text.split(/\r?\n/).filter(Boolean);
  const matchedLines = lines
    .filter((line) => FAILURE_PATTERN.test(line))
    .slice(-MAX_FAILURE_MATCHES);
  const tail = tailLines(text, MAX_FAILURE_TAIL_LINES);
  const tailSet = new Set(tail);
  const uniqueMatchedLines = matchedLines.filter((line) => !tailSet.has(line));
  const sections = [];

  if (uniqueMatchedLines.length > 0) {
    sections.push('--- matched failure lines ---', ...uniqueMatchedLines);
  }

  if (tail.length > 0) {
    sections.push('--- log tail ---', ...tail);
  }

  return sections.join('\n');
}

export function pruneRunDirectories(entries, maxKeep) {
  return [...entries]
    .sort((left, right) => right.mtimeMs - left.mtimeMs)
    .slice(maxKeep)
    .map((entry) => entry.path);
}

function getStages(mode) {
  if (mode === 'prepare') {
    return PREPARE_STAGES;
  }

  if (mode === 'validate') {
    return VALIDATE_STAGES;
  }

  throw new Error(`Unsupported mode '${mode}'. Use 'prepare' or 'validate'.`);
}

function ensureLogRoot() {
  fs.mkdirSync(logRootDir, { recursive: true });
}

function pruneOldRunDirectories() {
  ensureLogRoot();
  const entries = fs
    .readdirSync(logRootDir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => {
      const entryPath = path.join(logRootDir, entry.name);
      const stats = fs.statSync(entryPath);
      return { path: entryPath, mtimeMs: stats.mtimeMs };
    });

  for (const stalePath of pruneRunDirectories(entries, MAX_LOG_RUN_DIRS)) {
    fs.rmSync(stalePath, { recursive: true, force: true });
  }
}

function createRunDirectory(mode) {
  ensureLogRoot();
  pruneOldRunDirectories();

  const safeTimestamp = new Date().toISOString().replace(/[:.]/g, '-');
  const runDir = path.join(logRootDir, `${safeTimestamp}-${mode}`);
  fs.mkdirSync(runDir, { recursive: true });
  return runDir;
}

function buildStageLogPath(runDir, stageName, stageIndex) {
  return path.join(
    runDir,
    `${String(stageIndex + 1).padStart(2, '0')}-${sanitizeStageName(stageName)}.log`,
  );
}

function buildStageEnvironment() {
  return {
    ...process.env,
    CI: process.env.CI ?? '1',
    FORCE_COLOR: '0',
    NO_COLOR: '1',
    TERM: 'dumb',
  };
}

function appendNodeOption(existingValue, nextValue) {
  if (!existingValue || existingValue.trim().length === 0) {
    return nextValue;
  }

  return `${existingValue} ${nextValue}`;
}

function applyStageEnvironment(baseEnv, stage) {
  const env = { ...baseEnv };

  if (stage.env?.nodeHeapMb) {
    env.NODE_OPTIONS = appendNodeOption(
      env.NODE_OPTIONS,
      `--max-old-space-size=${stage.env.nodeHeapMb}`,
    );
  }

  return env;
}

function renderStageCommand(stage) {
  return `${stage.command} ${stage.args.join(' ')}`.trim();
}

async function runStage(stage, stageIndex, totalStages, runDir) {
  const stageLabel = `[${stageIndex + 1}/${totalStages}] ${stage.name}`;
  const logPath = buildStageLogPath(runDir, stage.name, stageIndex);
  const logStream = fs.createWriteStream(logPath, { flags: 'w' });
  const startedAt = Date.now();
  let child;

  console.log(`▶ ${stageLabel}`);

  try {
    child = spawn(stage.command, stage.args, {
      cwd: repoRoot,
      env: applyStageEnvironment(buildStageEnvironment(), stage),
      stdio: ['inherit', 'pipe', 'pipe'],
    });
  } catch (error) {
    logStream.end();
    throw error;
  }

  const heartbeat = setInterval(() => {
    console.log(
      `… ${stageLabel} still running (${formatDuration(Date.now() - startedAt)})`,
    );
  }, HEARTBEAT_INTERVAL_MS);

  const cleanup = () => {
    clearInterval(heartbeat);
  };

  const forwardSignal = (signal) => {
    if (child?.exitCode === null && !child.killed) {
      child.kill(signal);
    }
  };

  const sigintHandler = () => forwardSignal('SIGINT');
  const sigtermHandler = () => forwardSignal('SIGTERM');
  process.once('SIGINT', sigintHandler);
  process.once('SIGTERM', sigtermHandler);

  const finishLogging = async () => {
    await new Promise((resolve, reject) => {
      logStream.end((error) => {
        if (error) {
          reject(error);
          return;
        }

        resolve();
      });
    });
  };

  child.stdout.on('data', (chunk) => {
    logStream.write(chunk);
  });

  child.stderr.on('data', (chunk) => {
    logStream.write(chunk);
  });

  let closeResult;
  try {
    closeResult = await new Promise((resolve, reject) => {
      child.on('error', reject);
      child.on('close', (code, signal) => resolve({ code, signal }));
    });
  } finally {
    cleanup();
    process.off('SIGINT', sigintHandler);
    process.off('SIGTERM', sigtermHandler);
    await finishLogging();
  }

  const duration = formatDuration(Date.now() - startedAt);
  if (closeResult.code === 0) {
    console.log(`✓ ${stageLabel} (${duration})`);
    return;
  }

  const failureText = fs.readFileSync(logPath, 'utf8');
  const failureSummary = summarizeFailureText(failureText);
  const exitSummary =
    closeResult.code !== null
      ? `exit code ${closeResult.code}`
      : `signal ${closeResult.signal ?? 'unknown'}`;

  console.error(`✗ ${stageLabel} failed (${exitSummary}) after ${duration}`);
  console.error(`Command: ${renderStageCommand(stage)}`);
  if (failureSummary) {
    console.error(failureSummary);
  }
  console.error(`Full log: ${logPath}`);

  process.exit(closeResult.code ?? 1);
}

export async function runPipeline(mode) {
  const stages = getStages(mode);
  const runDir = createRunDirectory(mode);

  console.log(`Refactor ${mode} logs: ${runDir}`);

  for (const [index, stage] of stages.entries()) {
    await runStage(stage, index, stages.length, runDir);
  }

  console.log(`✅ refactor:${mode} completed`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === __filename) {
  const mode = process.argv[2] ?? 'validate';
  runPipeline(mode).catch((error) => {
    console.error(
      error instanceof Error ? (error.stack ?? error.message) : String(error),
    );
    process.exit(1);
  });
}
