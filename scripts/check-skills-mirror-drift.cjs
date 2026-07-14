/**
 * Ensures bundled skills stay in sync with .agents/skills source-of-truth copies.
 *
 * For each skill in MIRROR_MANIFEST, every file under .agents/skills/<name> must
 * exist in src-tauri/bundled_skills/<name> with identical content.
 * Extra files in bundled only are allowed.
 */
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const repoRoot = path.join(__dirname, '..');
const AGENTS_SKILLS_DIR = path.join(repoRoot, '.agents', 'skills');
const BUNDLED_SKILLS_DIR = path.join(repoRoot, 'src-tauri', 'bundled_skills');

/** Skills maintained in .agents and mirrored into bundled_skills */
const MIRROR_MANIFEST = ['setup-wizard'];

function sha256(filePath) {
  const data = fs.readFileSync(filePath);
  return crypto.createHash('sha256').update(data).digest('hex');
}

function listFilesRecursive(rootDir) {
  const files = [];
  if (!fs.existsSync(rootDir)) {
    return files;
  }

  function walk(currentDir) {
    for (const entry of fs.readdirSync(currentDir, { withFileTypes: true })) {
      const fullPath = path.join(currentDir, entry.name);
      if (entry.isDirectory()) {
        walk(fullPath);
      } else if (entry.isFile()) {
        files.push(path.relative(rootDir, fullPath).split(path.sep).join('/'));
      }
    }
  }

  walk(rootDir);
  return files.sort();
}

function checkMirroredSkill(skillName) {
  const sourceDir = path.join(AGENTS_SKILLS_DIR, skillName);
  const bundledDir = path.join(BUNDLED_SKILLS_DIR, skillName);
  const errors = [];

  if (!fs.existsSync(sourceDir)) {
    errors.push(`Source skill missing: .agents/skills/${skillName}`);
    return errors;
  }

  if (!fs.existsSync(bundledDir)) {
    errors.push(`Bundled skill missing: src-tauri/bundled_skills/${skillName}`);
    return errors;
  }

  const sourceFiles = listFilesRecursive(sourceDir);
  for (const relPath of sourceFiles) {
    const sourceFile = path.join(sourceDir, relPath);
    const bundledFile = path.join(bundledDir, ...relPath.split('/'));

    if (!fs.existsSync(bundledFile)) {
      errors.push(`Missing in bundled: ${skillName}/${relPath}`);
      continue;
    }

    const sourceHash = sha256(sourceFile);
    const bundledHash = sha256(bundledFile);
    if (sourceHash !== bundledHash) {
      errors.push(`Content drift: ${skillName}/${relPath}`);
    }
  }

  return errors;
}

function main() {
  const allErrors = [];

  for (const skillName of MIRROR_MANIFEST) {
    const errors = checkMirroredSkill(skillName);
    allErrors.push(...errors);
  }

  if (allErrors.length === 0) {
    console.log(
      `[OK] Skills mirror check passed for: ${MIRROR_MANIFEST.join(', ')}`,
    );
    process.exit(0);
  }

  console.error('[ERROR] Skills mirror drift detected:\n');
  for (const err of allErrors) {
    console.error(`  - ${err}`);
  }
  console.error(
    '\nFix: sync .agents/skills -> src-tauri/bundled_skills for mirrored skills.',
  );
  process.exit(1);
}

main();
