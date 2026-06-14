#!/usr/bin/env node
const fs = require('fs');
const path = require('path');

const repoRoot = path.join(__dirname, '..');
const gitDir = path.join(repoRoot, '.git');

// Skip if .git folder doesn't exist (e.g., inside Docker or CI without git repo context)
if (!fs.existsSync(gitDir)) {
  console.log('⚠️ .git folder not found. Skipping git hooks installation.');
  process.exit(0);
}

const hooksDir = path.join(gitDir, 'hooks');
if (!fs.existsSync(hooksDir)) {
  fs.mkdirSync(hooksDir, { recursive: true });
}

const preCommitHookPath = path.join(hooksDir, 'pre-commit');

const hookContent = `#!/bin/sh
# Git pre-commit hook to validate all bundled skills before commit
echo "🔍 Running bundled skills quality audit..."
pnpm skills:audit
RESULT=$?

if [ $RESULT -ne 0 ]; then
  echo "❌ Git commit aborted! Some bundled skills did not pass validation."
  echo "   Please resolve the skill warnings and score issues before committing."
  exit 1
fi

echo "✅ Bundled skills validation passed! Proceeding with commit."
exit 0
`;

try {
  fs.writeFileSync(preCommitHookPath, hookContent, {
    encoding: 'utf8',
    mode: 0o755,
  });
  if (process.platform !== 'win32') {
    fs.chmodSync(preCommitHookPath, 0o755);
  }
  console.log(
    '✅ Git pre-commit hook installed successfully! bundled_skills will be validated before every commit.',
  );
} catch (error) {
  console.error('❌ Failed to install Git pre-commit hook:', error.message);
}
