const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const versionType = process.argv[2];
if (!['patch', 'minor', 'major'].includes(versionType) && !/^\d+\.\d+\.\d+$/.test(versionType)) {
  console.error('Usage: node bump-version.js <patch|minor|major|version>');
  process.exit(1);
}

// 1. Bump package.json
const packageJsonPath = path.join(__dirname, '../package.json');
const packageJson = require(packageJsonPath);
let newVersion = versionType;

if (['patch', 'minor', 'major'].includes(versionType)) {
  const parts = packageJson.version.split('.').map(Number);
  if (versionType === 'major') {
    parts[0]++; parts[1] = 0; parts[2] = 0;
  } else if (versionType === 'minor') {
    parts[1]++; parts[2] = 0;
  } else {
    parts[2]++;
  }
  newVersion = parts.join('.');
}

packageJson.version = newVersion;
fs.writeFileSync(packageJsonPath, JSON.stringify(packageJson, null, 2) + '\n');
console.log(`Bumped package.json to ${newVersion}`);

// 2. Bump src-tauri/tauri.conf.json
const tauriConfPath = path.join(__dirname, '../src-tauri/tauri.conf.json');
const tauriConf = require(tauriConfPath);
tauriConf.version = newVersion;
fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n');
console.log(`Bumped tauri.conf.json to ${newVersion}`);

// 3. Bump src-tauri/Cargo.toml
const cargoTomlPath = path.join(__dirname, '../src-tauri/Cargo.toml');
let cargoToml = fs.readFileSync(cargoTomlPath, 'utf8');
// Replace version = "x.y.z" inside [package] block. 
// This is a simple regex, might need adjustment if Cargo.toml structure changes.
cargoToml = cargoToml.replace(/^version = "[^"]+"/m, `version = "${newVersion}"`);
fs.writeFileSync(cargoTomlPath, cargoToml);
console.log(`Bumped Cargo.toml to ${newVersion}`);

// 4. Bump aur/PKGBUILD
const pkgbuildPath = path.join(__dirname, '../aur/PKGBUILD');
let pkgbuild = fs.readFileSync(pkgbuildPath, 'utf8');
pkgbuild = pkgbuild.replace(/^pkgver=.+$/m, `pkgver=${newVersion}`);
// Reset pkgrel to 1
pkgbuild = pkgbuild.replace(/^pkgrel=.+$/m, `pkgrel=1`);
fs.writeFileSync(pkgbuildPath, pkgbuild);
console.log(`Bumped aur/PKGBUILD to ${newVersion}`);

// 5. Bump snap/snapcraft.yaml
const snapcraftPath = path.join(__dirname, '../snap/snapcraft.yaml');
if (fs.existsSync(snapcraftPath)) {
  let snapcraft = fs.readFileSync(snapcraftPath, 'utf8');
  snapcraft = snapcraft.replace(/^version: '.+'/m, `version: '${newVersion}'`);
  // Update source deb path
  // source: src-tauri/target/release/bundle/deb/libragent_0.3.15_amd64.deb
  snapcraft = snapcraft.replace(/libragent_[0-9]+\.[0-9]+\.[0-9]+_amd64\.deb/, `libragent_${newVersion}_amd64.deb`);
  fs.writeFileSync(snapcraftPath, snapcraft);
  console.log(`Bumped snap/snapcraft.yaml to ${newVersion}`);
}

console.log(newVersion); // Output the new version for the shell script to capture
