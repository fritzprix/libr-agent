#!/usr/bin/env node
/**
 * Validate bundled_assistants/ structure at build time.
 *
 * Checks:
 * 1. Each {assistant_name}/ has prompt.md
 * 2. Each {assistant_name}/ has mcp-config.json
 * 3. mcp-config.json fields are valid
 * 4. bundled_skills/ subdirectories have SKILL.md
 * 5. No orphaned include_str! in assistant_init.rs
 * 6. Every directory in bundled_assistants/ is embedded in assistant_init.rs
 */

const fs = require('fs');
const path = require('path');

const KNOWN_BUILTIN_SERVICES = new Set([
  'planning',
  'scratchpad',
  'workspace',
  'knowledge',
  'history',
  'dataset',
  'agent',
  'skills',
  'playbook',
  'attachments',
  'ui',
  'browser',
  'scheduled_task',
  'scheduled-task',
  'setup-wizard',
  'setup_wizard',
  'bootstrap',
  'tool',
  'media',
]);

function validate() {
  const root = path.join(__dirname, '..', 'src-tauri');
  const assistantsDir = path.join(root, 'bundled_assistants');
  const assistantInitPath = path.join(
    root,
    'src',
    'services',
    'assistant_init.rs',
  );

  let errors = [];
  let warnings = [];

  // 1. Check bundled_assistants/ exists
  if (!fs.existsSync(assistantsDir)) {
    console.error('❌ bundled_assistants/ directory does not exist');
    process.exit(1);
  }

  // 2. Enumerate assistant directories
  const assistantDirs = fs.readdirSync(assistantsDir).filter((name) => {
    const p = path.join(assistantsDir, name);
    return fs.statSync(p).isDirectory();
  });

  if (assistantDirs.length === 0) {
    console.error('❌ No assistant directories found in bundled_assistants/');
    process.exit(1);
  }

  const fileBasedNames = new Set(assistantDirs);

  // 3. Parse assistant_init.rs to verify compile-time embedding
  const initContent = fs.readFileSync(assistantInitPath, 'utf8');

  // Verify every directory in bundled_assistants/ is embedded
  for (const name of assistantDirs) {
    const expectedPath = `bundled_assistants/${name}/prompt.md`;
    if (!initContent.includes(expectedPath)) {
      errors.push(
        `Assistant "${name}" is defined in bundled_assistants/ but not embedded in assistant_init.rs via include_str!`,
      );
    }
  }

  // Check for orphaned include_str! statements in assistant_init.rs
  const includePattern =
    /include_str!\s*\(\s*["']\.\.\/\.\.\/bundled_assistants\/([^/"]+)\//g;
  let match;
  const foundEmbedDirs = new Set();
  while ((match = includePattern.exec(initContent)) !== null) {
    foundEmbedDirs.add(match[1]);
  }

  const orphans = [...foundEmbedDirs].filter(
    (name) => !fileBasedNames.has(name),
  );
  if (orphans.length > 0) {
    errors.push(
      `Orphaned include_str! statements in assistant_init.rs (refers to non-existent directories): ${orphans.join(', ')}`,
    );
  }

  // 4. Validate each assistant directory
  for (const assistantName of assistantDirs) {
    const assistantPath = path.join(assistantsDir, assistantName);

    // prompt.md
    const promptPath = path.join(assistantPath, 'prompt.md');
    if (!fs.existsSync(promptPath)) {
      errors.push(`${assistantName}/ missing prompt.md`);
      continue;
    }
    const promptContent = fs.readFileSync(promptPath, 'utf8');
    if (promptContent.trim().length < 50) {
      warnings.push(
        `${assistantName}/prompt.md seems too short (${promptContent.trim().length} chars)`,
      );
    }

    // mcp-config.json
    const configPath = path.join(assistantPath, 'mcp-config.json');
    if (!fs.existsSync(configPath)) {
      errors.push(`${assistantName}/ missing mcp-config.json`);
      continue;
    }
    try {
      const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));

      // Required fields
      if (
        !config.description ||
        typeof config.description !== 'string' ||
        config.description.trim().length === 0
      ) {
        errors.push(
          `${assistantName}/mcp-config.json: "description" is required and must be non-empty string`,
        );
      }
      if (!Array.isArray(config.allowedBuiltInServiceAliases)) {
        errors.push(
          `${assistantName}/mcp-config.json: "allowedBuiltInServiceAliases" is required and must be an array`,
        );
      } else {
        for (const alias of config.allowedBuiltInServiceAliases) {
          if (!KNOWN_BUILTIN_SERVICES.has(alias)) {
            errors.push(
              `${assistantName}/mcp-config.json: unknown or unauthorized builtin service alias "${alias}"`,
            );
          }
        }
      }
      if (
        config.mcpServerIds !== undefined &&
        !Array.isArray(config.mcpServerIds)
      ) {
        errors.push(
          `${assistantName}/mcp-config.json: "mcpServerIds" must be an array`,
        );
      }
      if (
        config.deletionProtected !== undefined &&
        typeof config.deletionProtected !== 'boolean'
      ) {
        errors.push(
          `${assistantName}/mcp-config.json: "deletionProtected" must be a boolean`,
        );
      }
      if (
        config.localServices !== undefined &&
        !Array.isArray(config.localServices)
      ) {
        errors.push(
          `${assistantName}/mcp-config.json: "localServices" must be an array`,
        );
      }
    } catch (e) {
      errors.push(
        `${assistantName}/mcp-config.json: invalid JSON — ${e.message}`,
      );
    }

    // bundled_skills/ (optional but validated if present)
    const skillsDir = path.join(assistantPath, 'bundled_skills');
    if (fs.existsSync(skillsDir)) {
      const skillDirs = fs.readdirSync(skillsDir).filter((name) => {
        const p = path.join(skillsDir, name);
        return fs.statSync(p).isDirectory();
      });
      for (const skillName of skillDirs) {
        const skillPath = path.join(skillsDir, skillName);
        if (!fs.existsSync(path.join(skillPath, 'SKILL.md'))) {
          errors.push(
            `${assistantName}/bundled_skills/${skillName}/ missing SKILL.md`,
          );
        }
      }
    }
  }

  // Report
  console.log('\n--- bundled_assistants/ Validation Report ---');
  console.log(`Checked ${assistantDirs.length} assistant directories`);

  if (warnings.length > 0) {
    console.log(`\n⚠️  ${warnings.length} warning(s):`);
    for (const w of warnings) console.log(`  ⚠️  ${w}`);
  }

  if (errors.length > 0) {
    console.log(`\n❌ ${errors.length} error(s):`);
    for (const e of errors) console.log(`  ❌ ${e}`);
    console.log('\nValidation FAILED. Fix errors before committing.\n');
    process.exit(1);
  }

  console.log('\n✅ Validation PASSED\n');
}

validate();
