const fs = require('fs');
const path = require('path');

const BUNDLED_SKILLS_DIR = path.join(__dirname, '../src-tauri/bundled_skills');

function auditSkill(skillName) {
  const skillPath = path.join(BUNDLED_SKILLS_DIR, skillName);
  const skillMdPath = path.join(skillPath, 'SKILL.md');
  const result = {
    name: skillName,
    hasSkillMd: false,
    frontmatterValid: true,
    noWhenToUseInBody: true,
    lineCount: 0,
    hasExtraneousFiles: false,
    hasReferences: false,
    warnings: [],
    score: 100,
  };

  if (!fs.existsSync(skillMdPath)) {
    result.warnings.push('SKILL.md is missing');
    result.score = 0;
    return result;
  }
  result.hasSkillMd = true;

  const content = fs.readFileSync(skillMdPath, 'utf8');
  const lines = content.split('\n');
  result.lineCount = lines.length;

  if (result.lineCount > 500) {
    result.warnings.push(
      `SKILL.md is too long (${result.lineCount} lines, limit is 500)`,
    );
    result.score -= 15;
  }

  // Parse Frontmatter
  const frontmatterMatch = content.match(/^---([\s\S]*?)---/);
  if (!frontmatterMatch) {
    result.warnings.push('YAML frontmatter is missing');
    result.frontmatterValid = false;
    result.score -= 20;
  } else {
    const fmText = frontmatterMatch[1];
    const keys = [];
    fmText.split('\n').forEach((line) => {
      const match = line.match(/^([a-zA-Z0-9_-]+)\s*:/);
      if (match) keys.push(match[1]);
    });

    if (!keys.includes('name')) {
      result.warnings.push('Frontmatter missing required "name" field');
      result.frontmatterValid = false;
      result.score -= 10;
    }
    if (!keys.includes('description')) {
      result.warnings.push('Frontmatter missing required "description" field');
      result.frontmatterValid = false;
      result.score -= 10;
    }

    const invalidKeys = keys.filter((k) => k !== 'name' && k !== 'description');
    if (invalidKeys.length > 0) {
      result.warnings.push(
        `Frontmatter contains extraneous keys: ${invalidKeys.join(', ')}`,
      );
      result.score -= 5;
    }
  }

  // Check body for "when to use" sections
  const bodyText = frontmatterMatch
    ? content.slice(frontmatterMatch[0].length)
    : content;
  const whenToUseHeaders = bodyText.match(
    /^#+\s*(when\s+to\s+use|triggers?)/im,
  );
  if (whenToUseHeaders) {
    result.warnings.push(
      '"When to use" or "Trigger" section found in Markdown body (should only be in frontmatter description)',
    );
    result.noWhenToUseInBody = false;
    result.score -= 15;
  }

  // Check extraneous files
  const files = fs.readdirSync(skillPath);
  const extraneousFiles = files.filter((f) => {
    const lower = f.toLowerCase();
    return (
      lower === 'readme.md' ||
      lower === 'changelog.md' ||
      lower === 'install.md' ||
      lower === 'installation.md'
    );
  });
  if (extraneousFiles.length > 0) {
    result.warnings.push(
      `Extraneous documentation files found: ${extraneousFiles.join(', ')}`,
    );
    result.hasExtraneousFiles = true;
    result.score -= 10;
  }

  // Check references
  const refPath = path.join(skillPath, 'references');
  if (fs.existsSync(refPath)) {
    const refFiles = fs.readdirSync(refPath).filter((f) => f.endsWith('.md'));
    if (refFiles.length > 0) {
      result.hasReferences = true;
      refFiles.forEach((refFile) => {
        if (!bodyText.includes(refFile)) {
          result.warnings.push(
            `Reference file "${refFile}" exists but is not linked in SKILL.md`,
          );
          result.score -= 5;
        }
      });
    }
  }

  result.score = Math.max(0, result.score);
  return result;
}

const skills = fs.readdirSync(BUNDLED_SKILLS_DIR).filter((f) => {
  return fs.statSync(path.join(BUNDLED_SKILLS_DIR, f)).isDirectory();
});

const auditResults = skills.map(auditSkill);

// Generate Markdown report
let report = '# Bundled Skills Quality Audit Report\n\n';
report += `Audited **${auditResults.length}** skills under \`src-tauri/bundled_skills/\` against \`/skill-creator\` design principles.\n\n`;

report += '## Overview Scoreboard\n\n';
report += '| Skill | Score | Line Count | References | Status | Warnings |\n';
report += '|---|---|---|---|---|---|\n';

let hasFailure = false;

auditResults.forEach((r) => {
  const statusIcon = r.score >= 90 ? '✅' : r.score >= 70 ? '⚠️' : '❌';
  const warningsText = r.warnings.join(', ') || 'None';
  report += `| [${r.name}](file://${path.join(BUNDLED_SKILLS_DIR, r.name)}) | ${r.score} | ${r.lineCount} | ${r.hasReferences ? 'Yes' : 'No'} | ${statusIcon} | ${warningsText} |\n`;

  // Fail condition: Any skill scored below 90 is considered malformed or has major guideline violations
  if (r.score < 90) {
    hasFailure = true;
  }
});

report += '\n## Detailed Findings by Skill\n\n';
auditResults
  .filter((r) => r.warnings.length > 0)
  .forEach((r) => {
    report += `### [${r.name}](file://${path.join(BUNDLED_SKILLS_DIR, r.name)}) (Score: ${r.score})\n`;
    r.warnings.forEach((w) => {
      report += `- ⚠️ ${w}\n`;
    });
    report += '\n';
  });

const shouldWriteFile = process.argv.includes('--write');
if (shouldWriteFile) {
  fs.writeFileSync(path.join(__dirname, '../audit_results.md'), report, 'utf8');
  console.log(
    'Audit completed successfully. Results written to audit_results.md',
  );
} else {
  console.log(report);
}

if (hasFailure) {
  console.error(
    '\n❌ Skill validation failed! Some skills scored below 90. Please check the warnings above.',
  );
  process.exit(1);
} else {
  console.log(
    '\n✅ All skills are valid and compliant with skill-creator guidelines.',
  );
  process.exit(0);
}
