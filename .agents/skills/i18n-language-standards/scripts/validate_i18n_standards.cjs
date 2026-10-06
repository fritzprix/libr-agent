#!/usr/bin/env node
/**
 * validate_i18n_standards.cjs
 *
 * Enforces ASD-STE100 (English) and Clear Korean UI language standards:
 * 1. Forbids internal developer identifiers (e.g. browser="userChrome", awaitAgent, pollProcess, createSession).
 * 2. Flags prohibited engineering jargon across locales (e.g. clean resample, circuit breaker, serving-engine, prefill).
 * 3. Checks JSON validity and key parity across all 8 locales.
 */

const fs = require('fs');
const path = require('path');

const LOCALES = ['en', 'ko', 'ja', 'zh', 'de', 'fr', 'es', 'pt'];
const REPO_ROOT = path.resolve(__dirname, '../../../..');
const LOCALES_DIR = path.join(REPO_ROOT, 'src', 'locales');
const SETTINGS_COMPONENTS_DIR = path.join(REPO_ROOT, 'src', 'features', 'settings');

// 1. Forbidden developer identifiers / internal code names that must NEVER appear in user-facing UI
const FORBIDDEN_IDENTIFIERS = [
  /browser=["']userChrome["']/i,
  /browser=["']sidecar["']/i,
  /\bawaitAgent\b/,
  /\bpollProcess\b/,
  /\bcreateSession\b/,
  /\bconda\.sh\b/,
  /\bnvm\.sh\b/,
  /\bZIP Slip\b/,
];

// 2. Prohibited engineering jargon in user-facing strings (Settings view)
const PROHIBITED_JARGON = [
  { pattern: /\bclean resample\b/i, label: 'clean resample (use "automatic retry")' },
  { pattern: /깨끗한 재샘플/, label: '깨끗한 재샘플 (사용 권장: "자동 재시도")' },
  { pattern: /\bcircuit breaker\b/i, label: 'circuit breaker (use "stops" or "stop task")' },
  { pattern: /서킷 브레이커/, label: '서킷 브레이커 (사용 권장: "작업 중단")' },
  { pattern: /\bserving-engine\b/i, label: 'serving-engine (use "model provider")' },
  { pattern: /서빙 엔진/, label: '서빙 엔진 (사용 권장: "모델 제공자")' },
  { pattern: /\bPrefill Performance\b/i, label: 'Prefill Performance (use "Initial Response Speed")' },
  { pattern: /프리필 성능/, label: '프리필 성능 (사용 권장: "첫 응답 반응 속도")' },
  { pattern: /\bold turns\b/i, label: 'old turns (use "older messages")' },
  { pattern: /Automation Governance/i, label: 'Automation Governance (use "Automation Rules")' },
  { pattern: /자동화 거버넌스/, label: '자동화 거버넌스 (사용 권장: "자동화 실행 규칙")' },
];

let hasErrors = false;

function error(msg) {
  console.error(`❌ [ERROR] ${msg}`);
  hasErrors = true;
}

function warn(msg) {
  console.warn(`⚠️ [WARN] ${msg}`);
}

function checkContent(filePath, content) {
  const relPath = path.relative(REPO_ROOT, filePath);
  
  // Check forbidden identifiers
  for (const regex of FORBIDDEN_IDENTIFIERS) {
    if (regex.test(content)) {
      error(`${relPath}: Contains forbidden internal identifier matching ${regex}`);
    }
  }

  // Check prohibited jargon
  for (const { pattern, label } of PROHIBITED_JARGON) {
    if (pattern.test(content)) {
      error(`${relPath}: Contains prohibited engineering jargon: ${label}`);
    }
  }
}

// Check all locale JSON files
console.log('🔍 Checking locale JSON files for language standards...');
const localeData = {};

for (const lang of LOCALES) {
  const jsonPath = path.join(LOCALES_DIR, lang, 'common.json');
  if (!fs.existsSync(jsonPath)) {
    error(`Missing locale file: ${jsonPath}`);
    continue;
  }

  const raw = fs.readFileSync(jsonPath, 'utf8');
  checkContent(jsonPath, raw);

  try {
    localeData[lang] = JSON.parse(raw);
  } catch (err) {
    error(`Failed to parse JSON for ${lang}: ${err.message}`);
  }
}

// Check key parity for settings block
if (localeData.en && localeData.ko) {
  function collectKeys(obj, prefix = '') {
    let keys = [];
    for (const [k, v] of Object.entries(obj)) {
      const full = prefix ? `${prefix}.${k}` : k;
      if (v && typeof v === 'object' && !Array.isArray(v)) {
        keys = keys.concat(collectKeys(v, full));
      } else {
        keys.push(full);
      }
    }
    return keys;
  }

  const enKeys = new Set(collectKeys(localeData.en.settings || {}));
  for (const lang of LOCALES) {
    if (!localeData[lang] || !localeData[lang].settings) continue;
    const currentKeys = new Set(collectKeys(localeData[lang].settings));
    const missing = [...enKeys].filter(k => !currentKeys.has(k));
    if (missing.length > 0) {
      warn(`${lang} is missing ${missing.length} settings keys compared to en (e.g. ${missing.slice(0, 3).join(', ')})`);
    }
  }
}

// Check Settings components for hardcoded forbidden strings in fallbacks
console.log('🔍 Checking Settings components fallback strings...');
function scanDir(dir) {
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      scanDir(fullPath);
    } else if (/\.(tsx|ts)$/.test(entry.name) && !entry.name.includes('.test.')) {
      const content = fs.readFileSync(fullPath, 'utf8');
      checkContent(fullPath, content);
    }
  }
}

if (fs.existsSync(SETTINGS_COMPONENTS_DIR)) {
  scanDir(SETTINGS_COMPONENTS_DIR);
}

if (hasErrors) {
  console.error('\n❌ Language standards enforcement failed. Please fix the above issues.');
  process.exit(1);
} else {
  console.log('\n✅ All locales and Settings components pass language standards validation!');
  process.exit(0);
}
