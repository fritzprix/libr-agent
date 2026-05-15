const fs = require('fs');
const path = require('path');

const versionType = process.argv[2];
if (
  !['patch', 'minor', 'major'].includes(versionType) &&
  !/^\d+\.\d+\.\d+$/.test(versionType)
) {
  console.error('Usage: node bump-version.js <patch|minor|major|version>');
  process.exit(1);
}

const REPO_OWNER = 'fritzprix';
const REPO_NAME = 'libr-agent';

// 1. Bump package.json
const packageJsonPath = path.join(__dirname, '../package.json');
const packageJson = require(packageJsonPath);
let newVersion = versionType;

if (['patch', 'minor', 'major'].includes(versionType)) {
  const parts = packageJson.version.split('.').map(Number);
  if (versionType === 'major') {
    parts[0]++;
    parts[1] = 0;
    parts[2] = 0;
  } else if (versionType === 'minor') {
    parts[1]++;
    parts[2] = 0;
  } else {
    parts[2]++;
  }
  newVersion = parts.join('.');
}

function buildReleaseUrl(tag, assetName) {
  return `https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/download/${tag}/${assetName}`;
}

function updateReadmeReleaseLinks(version) {
  const releaseTag = `v${version}`;
  const releasePageUrl = `https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/tag/${releaseTag}`;
  const assets = {
    windowsExe: `LibrAgent_${version}_x64-setup.exe`,
    windowsMsi: `LibrAgent_${version}_x64_en-US.msi`,
    macosDmg: `LibrAgent_${version}_aarch64.dmg`,
    linuxAppImage: `LibrAgent_${version}_amd64.AppImage`,
    linuxDeb: `LibrAgent_${version}_amd64.deb`,
    linuxRpm: `LibrAgent-${version}-1.x86_64.rpm`,
  };

  const readmeConfigs = [
    {
      file: 'README.md',
      lines: [
        `- **Windows:** [\`${assets.windowsExe}\`](${buildReleaseUrl(releaseTag, assets.windowsExe)}) · [\`${assets.windowsMsi}\`](${buildReleaseUrl(releaseTag, assets.windowsMsi)})`,
        `- **macOS (Apple Silicon):** [\`${assets.macosDmg}\`](${buildReleaseUrl(releaseTag, assets.macosDmg)})`,
        `- **Linux:** [\`${assets.linuxAppImage}\`](${buildReleaseUrl(releaseTag, assets.linuxAppImage)}) · [\`${assets.linuxDeb}\`](${buildReleaseUrl(releaseTag, assets.linuxDeb)}) · [\`${assets.linuxRpm}\`](${buildReleaseUrl(releaseTag, assets.linuxRpm)})`,
        `- **All release assets:** [Releases page](${releasePageUrl})`,
      ],
    },
    {
      file: 'README.ko.md',
      lines: [
        `- **Windows:** [\`${assets.windowsExe}\`](${buildReleaseUrl(releaseTag, assets.windowsExe)}) · [\`${assets.windowsMsi}\`](${buildReleaseUrl(releaseTag, assets.windowsMsi)})`,
        `- **macOS (Apple Silicon):** [\`${assets.macosDmg}\`](${buildReleaseUrl(releaseTag, assets.macosDmg)})`,
        `- **Linux:** [\`${assets.linuxAppImage}\`](${buildReleaseUrl(releaseTag, assets.linuxAppImage)}) · [\`${assets.linuxDeb}\`](${buildReleaseUrl(releaseTag, assets.linuxDeb)}) · [\`${assets.linuxRpm}\`](${buildReleaseUrl(releaseTag, assets.linuxRpm)})`,
        `- **전체 릴리스 자산:** [릴리스 페이지](${releasePageUrl})`,
      ],
    },
    {
      file: 'README.zh.md',
      lines: [
        `- **Windows：** [\`${assets.windowsExe}\`](${buildReleaseUrl(releaseTag, assets.windowsExe)}) · [\`${assets.windowsMsi}\`](${buildReleaseUrl(releaseTag, assets.windowsMsi)})`,
        `- **macOS（Apple Silicon）：** [\`${assets.macosDmg}\`](${buildReleaseUrl(releaseTag, assets.macosDmg)})`,
        `- **Linux：** [\`${assets.linuxAppImage}\`](${buildReleaseUrl(releaseTag, assets.linuxAppImage)}) · [\`${assets.linuxDeb}\`](${buildReleaseUrl(releaseTag, assets.linuxDeb)}) · [\`${assets.linuxRpm}\`](${buildReleaseUrl(releaseTag, assets.linuxRpm)})`,
        `- **完整发布资源：** [发布页面](${releasePageUrl})`,
      ],
    },
    {
      file: 'README.ja.md',
      lines: [
        `- **Windows:** [\`${assets.windowsExe}\`](${buildReleaseUrl(releaseTag, assets.windowsExe)}) · [\`${assets.windowsMsi}\`](${buildReleaseUrl(releaseTag, assets.windowsMsi)})`,
        `- **macOS (Apple Silicon):** [\`${assets.macosDmg}\`](${buildReleaseUrl(releaseTag, assets.macosDmg)})`,
        `- **Linux:** [\`${assets.linuxAppImage}\`](${buildReleaseUrl(releaseTag, assets.linuxAppImage)}) · [\`${assets.linuxDeb}\`](${buildReleaseUrl(releaseTag, assets.linuxDeb)}) · [\`${assets.linuxRpm}\`](${buildReleaseUrl(releaseTag, assets.linuxRpm)})`,
        `- **すべてのリリース資産:** [リリースページ](${releasePageUrl})`,
      ],
    },
    {
      file: 'README.fr.md',
      lines: [
        `- **Windows :** [\`${assets.windowsExe}\`](${buildReleaseUrl(releaseTag, assets.windowsExe)}) · [\`${assets.windowsMsi}\`](${buildReleaseUrl(releaseTag, assets.windowsMsi)})`,
        `- **macOS (Apple Silicon) :** [\`${assets.macosDmg}\`](${buildReleaseUrl(releaseTag, assets.macosDmg)})`,
        `- **Linux :** [\`${assets.linuxAppImage}\`](${buildReleaseUrl(releaseTag, assets.linuxAppImage)}) · [\`${assets.linuxDeb}\`](${buildReleaseUrl(releaseTag, assets.linuxDeb)}) · [\`${assets.linuxRpm}\`](${buildReleaseUrl(releaseTag, assets.linuxRpm)})`,
        `- **Tous les fichiers de release :** [page des Releases](${releasePageUrl})`,
      ],
    },
    {
      file: 'README.es.md',
      lines: [
        `- **Windows:** [\`${assets.windowsExe}\`](${buildReleaseUrl(releaseTag, assets.windowsExe)}) · [\`${assets.windowsMsi}\`](${buildReleaseUrl(releaseTag, assets.windowsMsi)})`,
        `- **macOS (Apple Silicon):** [\`${assets.macosDmg}\`](${buildReleaseUrl(releaseTag, assets.macosDmg)})`,
        `- **Linux:** [\`${assets.linuxAppImage}\`](${buildReleaseUrl(releaseTag, assets.linuxAppImage)}) · [\`${assets.linuxDeb}\`](${buildReleaseUrl(releaseTag, assets.linuxDeb)}) · [\`${assets.linuxRpm}\`](${buildReleaseUrl(releaseTag, assets.linuxRpm)})`,
        `- **Todos los archivos de la release:** [página de Releases](${releasePageUrl})`,
      ],
    },
    {
      file: 'README.de.md',
      lines: [
        `- **Windows:** [\`${assets.windowsExe}\`](${buildReleaseUrl(releaseTag, assets.windowsExe)}) · [\`${assets.windowsMsi}\`](${buildReleaseUrl(releaseTag, assets.windowsMsi)})`,
        `- **macOS (Apple Silicon):** [\`${assets.macosDmg}\`](${buildReleaseUrl(releaseTag, assets.macosDmg)})`,
        `- **Linux:** [\`${assets.linuxAppImage}\`](${buildReleaseUrl(releaseTag, assets.linuxAppImage)}) · [\`${assets.linuxDeb}\`](${buildReleaseUrl(releaseTag, assets.linuxDeb)}) · [\`${assets.linuxRpm}\`](${buildReleaseUrl(releaseTag, assets.linuxRpm)})`,
        `- **Alle Release-Artefakte:** [Releases-Seite](${releasePageUrl})`,
      ],
    },
    {
      file: 'README.pt.md',
      lines: [
        `- **Windows:** [\`${assets.windowsExe}\`](${buildReleaseUrl(releaseTag, assets.windowsExe)}) · [\`${assets.windowsMsi}\`](${buildReleaseUrl(releaseTag, assets.windowsMsi)})`,
        `- **macOS (Apple Silicon):** [\`${assets.macosDmg}\`](${buildReleaseUrl(releaseTag, assets.macosDmg)})`,
        `- **Linux:** [\`${assets.linuxAppImage}\`](${buildReleaseUrl(releaseTag, assets.linuxAppImage)}) · [\`${assets.linuxDeb}\`](${buildReleaseUrl(releaseTag, assets.linuxDeb)}) · [\`${assets.linuxRpm}\`](${buildReleaseUrl(releaseTag, assets.linuxRpm)})`,
        `- **Todos os artefatos da release:** [página de Releases](${releasePageUrl})`,
      ],
    },
  ];

  for (const { file, lines } of readmeConfigs) {
    const readmePath = path.join(__dirname, `../${file}`);
    const readme = fs.readFileSync(readmePath, 'utf8');
    const replacement = [
      '<!-- RELEASE_DOWNLOADS_START -->',
      ...lines,
      '<!-- RELEASE_DOWNLOADS_END -->',
    ].join('\n');

    if (!readme.includes('<!-- RELEASE_DOWNLOADS_START -->')) {
      throw new Error(`Missing RELEASE_DOWNLOADS_START marker in ${file}`);
    }

    const updatedReadme = readme.replace(
      /<!-- RELEASE_DOWNLOADS_START -->[\s\S]*?<!-- RELEASE_DOWNLOADS_END -->/,
      replacement,
    );

    fs.writeFileSync(readmePath, updatedReadme);
    console.log(`Updated ${file} release download links to ${releaseTag}`);
  }
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
cargoToml = cargoToml.replace(
  /^version = "[^"]+"/m,
  `version = "${newVersion}"`,
);
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
  snapcraft = snapcraft.replace(
    /libragent_[0-9]+\.[0-9]+\.[0-9]+_amd64\.deb/,
    `libragent_${newVersion}_amd64.deb`,
  );
  fs.writeFileSync(snapcraftPath, snapcraft);
  console.log(`Bumped snap/snapcraft.yaml to ${newVersion}`);
}

// 6. Refresh README direct download links for the new release tag
updateReadmeReleaseLinks(newVersion);

console.log(newVersion); // Output the new version for the shell script to capture
