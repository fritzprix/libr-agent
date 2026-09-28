/**
 * AppImageHub-compatible Linux build: shared Microsoft ONNX Runtime + overlay
 * config. Prefer this (or release CI) over plain `pnpm tauri build` when you
 * need a glibc 2.35 AppImage.
 */
const { execSync } = require('node:child_process');
const path = require('node:path');

const repoRoot = path.resolve(__dirname, '..');
const libDir = path.join(repoRoot, 'src-tauri', 'onnxruntime-linux', 'lib');

const env = {
  ...process.env,
  ORT_LIB_PATH: libDir,
  ORT_PREFER_DYNAMIC_LINK: '1',
  LD_LIBRARY_PATH: [libDir, process.env.LD_LIBRARY_PATH]
    .filter(Boolean)
    .join(path.delimiter),
};

const extraArgs = process.argv.slice(2).join(' ');
const command = [
  'pnpm exec tauri build',
  '--config src-tauri/tauri.appimage-ort.conf.json',
  extraArgs,
]
  .filter(Boolean)
  .join(' ');

execSync(command, { cwd: repoRoot, stdio: 'inherit', env });
