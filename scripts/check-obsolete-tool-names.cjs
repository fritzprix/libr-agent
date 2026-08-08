#!/usr/bin/env node
/**
 * Fail CI when obsolete / non-invocable builtin tool names appear in
 * agent-facing guidance (recovery hints, tool descriptions, bundled skills, docs).
 *
 * Contract: agents must invoke tools as `server__tool` (see route_tool).
 * Obsolete short forms like `agent__list` / `tool__list` must not resurface.
 */

const fs = require('fs');
const path = require('path');

const repoRoot = path.resolve(__dirname, '..');

/** Patterns that are always wrong in agent-facing prose. */
const FORBIDDEN = [
  {
    id: 'obsolete-agent-list',
    // agent__list( but not agent__listAgents
    re: /\bagent__list(?!Agents)\b/,
    message: 'Use agent__listAgents (not obsolete agent__list)',
  },
  {
    id: 'obsolete-agent-create',
    // agent__create( but not agent__createAgent / agent__createOrg
    re: /\bagent__create(?!Agent|Org)\b/,
    message: 'Use agent__createAgent (not obsolete agent__create)',
  },
  {
    id: 'obsolete-agent-update',
    // agent__update( but not agent__updateAgent
    re: /\bagent__update(?!Agent)\b/,
    message: 'Use agent__updateAgent (not obsolete agent__update)',
  },
  {
    id: 'obsolete-tool-list',
    // tool__list but not tool__listServers
    re: /\btool__list(?!Servers)\b/,
    message: 'Use tool__listServers (not obsolete tool__list)',
  },
  {
    id: 'obsolete-tool-register',
    re: /\btool__register(?!Server)\b/,
    message: 'Use tool__registerServer (not obsolete tool__register)',
  },
  {
    id: 'obsolete-history-list',
    re: /\bhistory__list(?!Sessions)\b/,
    message: 'Use history__listSessions (not obsolete history__list)',
  },
  {
    id: 'obsolete-history-search',
    re: /\bhistory__search(?!History)\b/,
    message: 'Use history__searchHistory (not obsolete history__search)',
  },
  {
    id: 'snake-knowledge-record',
    re: /\bknowledge__record_knowledge\b/,
    message:
      'Use knowledge__recordKnowledge (not snake_case knowledge__record_knowledge)',
  },
  {
    id: 'snake-knowledge-search',
    re: /\bknowledge__search_knowledge\b/,
    message:
      'Use knowledge__searchKnowledge (not snake_case knowledge__search_knowledge)',
  },
  {
    id: 'bare-use-listAgents',
    re: /\bUse listAgents\b/,
    message: 'Use agent__listAgents (bare listAgents is not invocable)',
  },
  {
    id: 'bare-use-listSessions',
    re: /\bUse listSessions\b/,
    message: 'Use history__listSessions or agent__listAgents(type="sessions")',
  },
  {
    id: 'bare-use-listServers',
    re: /\bUse listServers\b/,
    message: 'Use tool__listServers (bare listServers is not invocable)',
  },
  {
    id: 'bare-use-checkSession',
    re: /\bUse checkSession\b/,
    message: 'Use agent__checkSession (bare checkSession is not invocable)',
  },
  {
    id: 'bare-use-startSession',
    re: /\bUse startSession\b/,
    message: 'Use agent__startSession (bare startSession is not invocable)',
  },
  {
    id: 'bare-use-updateAgent',
    re: /\bUse updateAgent\b/,
    message: 'Use agent__updateAgent (bare updateAgent is not invocable)',
  },
  {
    id: 'bare-use-updateServer',
    re: /\bUse updateServer\b/,
    message: 'Use tool__updateServer (bare updateServer is not invocable)',
  },
  {
    id: 'bare-use-getAgentStatus',
    re: /\b(Use|using|with) getAgentStatus\b/,
    message: 'Use agent__checkSession (getAgentStatus is obsolete)',
  },
  {
    id: 'bare-use-awaitAgent',
    re: /\bUse awaitAgent\b/,
    message: 'Use agent__checkSession (awaitAgent is obsolete)',
  },
  {
    id: 'bare-use-listAssistants',
    re: /\bUse listAssistants\b/,
    message: 'Use agent__listAgents (listAssistants is obsolete)',
  },
  {
    id: 'bare-use-getChildAgents',
    re: /\buse getChildAgents\b/i,
    message:
      'Use agent__listAgents(type="sessions") (getChildAgents is obsolete)',
  },
  {
    id: 'bare-use-getCurrentUrl',
    re: /\bUse getCurrentUrl\b/,
    message: 'Use browser__getCurrentUrl (bare getCurrentUrl is not invocable)',
  },
  {
    id: 'bare-use-getConsoleLogs',
    re: /\bUse getConsoleLogs\b/,
    message:
      'Use browser__getConsoleLogs (bare getConsoleLogs is not invocable)',
  },
  {
    id: 'bare-use-evaluateJS',
    re: /\bUse evaluateJS\b/,
    message: 'Use browser__evaluateJS (bare evaluateJS is not invocable)',
  },
  {
    id: 'bare-use-stopProcess',
    re: /\bUse stopProcess\b/,
    message: 'Use workspace__stopProcess (bare stopProcess is not invocable)',
  },
  {
    id: 'bare-use-runInPersistentShell',
    re: /\bUse runInPersistentShell\b/,
    message: 'Use workspace__runInPersistentShell (bare name is not invocable)',
  },
  {
    id: 'bare-use-runInPersistentPowerShell',
    re: /\bUse runInPersistentPowerShell\b/,
    message:
      'Use workspace__runInPersistentPowerShell (bare name is not invocable)',
  },
  {
    id: 'bare-use-updateGoal',
    re: /\bUse updateGoal\b/,
    message: 'Use planning__updateGoal (bare updateGoal is not invocable)',
  },
  {
    id: 'bare-use-reflect',
    re: /\bUse reflect\b/,
    message: 'Use planning__reflect (bare reflect is not invocable)',
  },
  {
    id: 'bare-use-readSession',
    re: /\bUse readSession\b/,
    message: 'Use history__readSession (bare readSession is not invocable)',
  },
  {
    id: 'bare-use-readMessage',
    re: /\bUse readMessage\b/,
    message: 'Use history__readMessage (bare readMessage is not invocable)',
  },
  {
    id: 'bare-use-waitForProcess',
    re: /\bUse waitForProcess\b/,
    message:
      'Use workspace__waitForProcess (bare waitForProcess is not invocable)',
  },
  {
    id: 'bare-use-readProcessOutput',
    re: /\bUse readProcessOutput\b/,
    message:
      'Use workspace__readProcessOutput (bare readProcessOutput is not invocable)',
  },
  {
    id: 'bare-use-messageToSession',
    re: /\bUse messageToSession\b/,
    message:
      'Use agent__messageToSession (bare messageToSession is not invocable)',
  },
  {
    id: 'bare-createOrg-invoke',
    // createOrg( but not agent__createOrg(
    re: /(?<!agent__)createOrg\s*\(/,
    message: 'Use agent__createOrg(...) (bare createOrg is not invocable)',
    onlyUnder: [
      'src-tauri/src/mcp',
      'src-tauri/bundled_skills',
      'src-tauri/tests/integration',
      '.agents/skills',
    ],
  },
  {
    id: 'bare-createOrg-prose',
    re: /\b(Use|Call|call|Then call|before)\s+createOrg\b/,
    message: 'Use agent__createOrg (bare createOrg is not invocable)',
    onlyUnder: [
      'src-tauri/src/mcp',
      'src-tauri/bundled_skills',
      'src-tauri/tests/integration',
      '.agents/skills',
    ],
  },
  {
    id: 'bare-createOrg-sentence',
    re: /(?<!agent__)createOrg (requires|must)\b/,
    message: 'Use agent__createOrg in agent-facing error/guidance prose',
    onlyUnder: [
      'src-tauri/src/mcp',
      'src-tauri/bundled_skills',
      'src-tauri/tests/integration',
      '.agents/skills',
    ],
  },
  {
    id: 'bare-getOrg-invoke',
    re: /(?<!agent__)getOrg\s*\(/,
    message: 'Use agent__getOrg(...) (bare getOrg is not invocable)',
    onlyUnder: [
      'src-tauri/src/mcp',
      'src-tauri/bundled_skills',
      'src-tauri/tests/integration',
      '.agents/skills',
    ],
  },
  {
    id: 'bare-getOrg-prose',
    re: /\b(Use|with|by|previous)\s+getOrg\b/,
    message: 'Use agent__getOrg (bare getOrg is not invocable)',
    onlyUnder: [
      'src-tauri/src/mcp',
      'src-tauri/bundled_skills',
      'src-tauri/tests/integration',
      '.agents/skills',
    ],
  },
  {
    id: 'bare-prepareTeamworkWorkspace-invoke',
    re: /(?<!agent__)prepareTeamworkWorkspace\s*\(/,
    message:
      'Use agent__prepareTeamworkWorkspace(...) (bare name is not invocable)',
    onlyUnder: [
      'src-tauri/src/mcp',
      'src-tauri/bundled_skills',
      'src-tauri/tests/integration',
      '.agents/skills',
    ],
  },
  {
    id: 'bare-prepareTeamworkWorkspace-prose',
    re: /\b(Use|Call|call|Then call|Do not call|with)\s+prepareTeamworkWorkspace\b/,
    message: 'Use agent__prepareTeamworkWorkspace (bare name is not invocable)',
    onlyUnder: [
      'src-tauri/src/mcp',
      'src-tauri/bundled_skills',
      'src-tauri/tests/integration',
      '.agents/skills',
    ],
  },
  {
    id: 'bare-prepareTeamworkWorkspace-sentence',
    re: /(?<!agent__)prepareTeamworkWorkspace (must|again)\b/,
    message: 'Use agent__prepareTeamworkWorkspace in agent-facing prose',
    onlyUnder: [
      'src-tauri/src/mcp',
      'src-tauri/bundled_skills',
      'src-tauri/tests/integration',
      '.agents/skills',
    ],
  },
  {
    id: 'obsolete-cancel-session-scheduled-task',
    // Valid as a Tauri command name in docs/commands; forbid only in agent-facing MCP prose.
    re: /\bcancel_session_scheduled_task\b/,
    message:
      'Use scheduled_task__deleteScheduledTask (cancel_session_scheduled_task is obsolete as an MCP tool name)',
    onlyUnder: [
      'src-tauri/src/mcp',
      'src-tauri/bundled_skills',
      'src-tauri/tests/integration',
      '.agents/skills',
    ],
  },
  {
    id: 'bare-search-showLineAnchors',
    re: /(?<!workspace__)search\(showLineAnchors=true\)/,
    message: 'Use workspace__searchFiles(showLineAnchors=true)',
    onlyUnder: [
      'src-tauri/src/mcp',
      'src-tauri/bundled_skills',
      'src-tauri/tests/integration',
      '.agents/skills',
    ],
  },
];

const SCAN_GLOBS = [
  'src-tauri/src/mcp',
  'src-tauri/bundled_skills',
  'src-tauri/tests/integration',
  'docs',
  'README.md',
  'README.ko.md',
  'README.ja.md',
  'README.zh.md',
  'README.es.md',
  'README.pt.md',
  'README.fr.md',
  'README.de.md',
  '.agents/skills',
];

const ALLOWED_EXTENSIONS = new Set([
  '.rs',
  '.md',
  '.cjs',
  '.js',
  '.ts',
  '.tsx',
]);

/** Historical / generated paths that may still mention obsolete names. */
const IGNORE_PATH_PARTS = [
  `${path.sep}CHANGELOG.md`,
  `${path.sep}docs${path.sep}sprints${path.sep}`,
  `${path.sep}node_modules${path.sep}`,
  `${path.sep}target${path.sep}`,
];

function shouldIgnore(filePath) {
  const normalized = path.normalize(filePath);
  return IGNORE_PATH_PARTS.some((part) => normalized.includes(part));
}

function walk(dir, acc) {
  if (!fs.existsSync(dir)) return;
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (shouldIgnore(full)) continue;
    if (entry.isDirectory()) {
      if (
        entry.name === 'node_modules' ||
        entry.name === 'target' ||
        entry.name === '.git'
      ) {
        continue;
      }
      walk(full, acc);
    } else if (entry.isFile()) {
      const ext = path.extname(entry.name);
      if (ALLOWED_EXTENSIONS.has(ext)) {
        acc.push(full);
      }
    }
  }
}

function collectFiles() {
  const files = [];
  for (const rel of SCAN_GLOBS) {
    const abs = path.join(repoRoot, rel);
    if (!fs.existsSync(abs)) continue;
    const stat = fs.statSync(abs);
    if (stat.isFile()) {
      if (!shouldIgnore(abs)) files.push(abs);
    } else {
      walk(abs, files);
    }
  }
  return files;
}

function ruleAppliesToFile(rule, file) {
  if (!rule.onlyUnder || rule.onlyUnder.length === 0) {
    return true;
  }
  const rel = path.relative(repoRoot, file).split(path.sep).join('/');
  return rule.onlyUnder.some(
    (prefix) => rel === prefix || rel.startsWith(`${prefix}/`),
  );
}

function main() {
  const files = collectFiles();
  const violations = [];

  for (const file of files) {
    const content = fs.readFileSync(file, 'utf8');
    const lines = content.split(/\r?\n/);
    for (let i = 0; i < lines.length; i += 1) {
      const line = lines[i];
      for (const rule of FORBIDDEN) {
        if (!ruleAppliesToFile(rule, file)) continue;
        if (rule.re.test(line)) {
          violations.push({
            file: path.relative(repoRoot, file),
            line: i + 1,
            id: rule.id,
            message: rule.message,
            excerpt: line.trim().slice(0, 160),
          });
        }
      }
    }
  }

  if (violations.length === 0) {
    console.log(
      `check-obsolete-tool-names: OK (${files.length} files scanned, 0 violations)`,
    );
    process.exit(0);
  }

  console.error(
    `check-obsolete-tool-names: ${violations.length} violation(s) in ${files.length} files:\n`,
  );
  for (const v of violations) {
    console.error(`  ${v.file}:${v.line} [${v.id}] ${v.message}`);
    console.error(`    ${v.excerpt}`);
  }
  process.exit(1);
}

main();
