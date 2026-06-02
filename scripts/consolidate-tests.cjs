const fs = require('fs');
const path = require('path');

const TESTS_DIR = path.join(__dirname, '..', 'src-tauri', 'tests');
const INTEGRATION_DIR = path.join(TESTS_DIR, 'integration');
const COMMON_DIR = path.join(TESTS_DIR, 'common');

function main() {
  console.log(`Starting test consolidation...`);
  console.log(`Tests directory: ${TESTS_DIR}`);
  console.log(`Integration directory: ${INTEGRATION_DIR}`);
  console.log(`Common directory: ${COMMON_DIR}`);

  // Create integration directory if it doesn't exist
  if (!fs.existsSync(INTEGRATION_DIR)) {
    fs.mkdirSync(INTEGRATION_DIR, { recursive: true });
    console.log(`Created directory: ${INTEGRATION_DIR}`);
  }

  // Create common directory if it doesn't exist
  if (!fs.existsSync(COMMON_DIR)) {
    fs.mkdirSync(COMMON_DIR, { recursive: true });
    console.log(`Created directory: ${COMMON_DIR}`);
  }

  // Move common.rs to common/mod.rs if common.rs exists at the root
  const commonRsPath = path.join(TESTS_DIR, 'common.rs');
  const commonModPath = path.join(COMMON_DIR, 'mod.rs');
  if (fs.existsSync(commonRsPath)) {
    fs.renameSync(commonRsPath, commonModPath);
    console.log(`Moved common.rs to common/mod.rs`);
  } else {
    console.log(
      `common.rs does not exist at the root (might have already been moved or not present). Checking if common/mod.rs exists...`,
    );
    if (!fs.existsSync(commonModPath)) {
      console.error(`ERROR: Neither common.rs nor common/mod.rs exists!`);
      process.exit(1);
    }
  }

  // Read all files from TESTS_DIR
  const files = fs.readdirSync(TESTS_DIR);
  const rustTestFiles = [];

  for (const file of files) {
    const filePath = path.join(TESTS_DIR, file);
    const stat = fs.statSync(filePath);

    if (stat.isFile() && file.endsWith('.rs')) {
      if (file === 'common.rs' || file === 'integration_tests.rs') {
        console.log(`Skipping special file: ${file}`);
        continue;
      }
      rustTestFiles.push(file);
    }
  }

  console.log(`Found ${rustTestFiles.length} Rust test files to consolidate.`);

  const modules = [];

  for (const file of rustTestFiles) {
    const oldPath = path.join(TESTS_DIR, file);
    const newPath = path.join(INTEGRATION_DIR, file);
    const moduleName = file.slice(0, -3); // remove .rs

    // Read content
    let content = fs.readFileSync(oldPath, 'utf8');

    // Replace mod common; with use crate::common;
    // Handle variations in spacing and annotations
    content = content.replace(
      /^\s*(pub\s+)?(pub\(crate\)\s+)?mod\s+common\s*;/gm,
      'use crate::common;',
    );

    // Handle relative path attributes for build_support
    // #[path = "../build_support/bundled_skills.rs"]
    content = content.replace(
      /#\[path\s*=\s*"..\/build_support\//g,
      '#[path = "../../build_support/',
    );

    // Write to new location
    fs.writeFileSync(newPath, content, 'utf8');

    // Remove old file
    fs.unlinkSync(oldPath);

    console.log(`Consolidated: ${file} -> integration/${file}`);
    modules.push(moduleName);
  }

  // Sort modules for consistent mod.rs ordering
  modules.sort();

  // Create integration/mod.rs
  const modRsPath = path.join(INTEGRATION_DIR, 'mod.rs');
  let modRsContent =
    '// Auto-generated module declarations for LibrAgent integration tests\n\n';
  for (const mod of modules) {
    modRsContent += `pub mod ${mod};\n`;
  }
  fs.writeFileSync(modRsPath, modRsContent, 'utf8');
  console.log(`Created: ${modRsPath}`);

  // Create tests/integration_tests.rs
  const integrationTestsRsPath = path.join(TESTS_DIR, 'integration_tests.rs');
  const integrationTestsRsContent = `// LibrAgent Consolidated Integration Tests
mod common;
mod integration;
`;
  fs.writeFileSync(integrationTestsRsPath, integrationTestsRsContent, 'utf8');
  console.log(`Created: ${integrationTestsRsPath}`);

  console.log(`Test consolidation complete!`);
}

main();
