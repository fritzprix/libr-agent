import { readFile, writeFile } from 'fs/promises';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const rootDir = join(__dirname, '..');

async function syncVersion() {
  const packageJsonPath = join(rootDir, 'package.json');
  const tauriConfPath = join(rootDir, 'src-tauri', 'tauri.conf.json');
  const cargoTomlPath = join(rootDir, 'src-tauri', 'Cargo.toml');

  const packageJson = JSON.parse(await readFile(packageJsonPath, 'utf-8'));
  const version = packageJson.version;

  console.log(`Syncing version ${version} to Tauri configuration...`);

  // Update tauri.conf.json
  try {
    const tauriConf = JSON.parse(await readFile(tauriConfPath, 'utf-8'));
    if (tauriConf.version !== version) {
      tauriConf.version = version;
      await writeFile(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n');
      console.log(`Updated tauri.conf.json to ${version}`);
    } else {
      console.log('tauri.conf.json is already up to date.');
    }
  } catch (error) {
    console.error('Error updating tauri.conf.json:', error);
  }

  // Update Cargo.toml
  try {
    let cargoToml = await readFile(cargoTomlPath, 'utf-8');
    // Match version = "x.y.z" in the [package] section
    // This regex is a bit simple, it assumes version is near the top or we just replace the first occurrence which is usually package version
    // A safer way is to look for [package] then version
    const versionRegex = /^version = ".*"$/m;

    if (versionRegex.test(cargoToml)) {
      const currentMatch = cargoToml.match(versionRegex)[0];
      const newVersionLine = `version = "${version}"`;

      if (currentMatch !== newVersionLine) {
        cargoToml = cargoToml.replace(versionRegex, newVersionLine);
        await writeFile(cargoTomlPath, cargoToml);
        console.log(`Updated Cargo.toml to ${version}`);
      } else {
        console.log('Cargo.toml is already up to date.');
      }
    } else {
      console.warn('Could not find version field in Cargo.toml');
    }
  } catch (error) {
    console.error('Error updating Cargo.toml:', error);
  }
}

syncVersion().catch(console.error);
