/**
 * Frontend build, then optionally (Linux x64 + --appimage-ort) stage Microsoft's
 * ONNX Runtime shared library for AppImageHub-compatible bundles.
 *
 * Local `pnpm tauri build` uses pyke's static ORT and skips staging/sudo.
 * Release CI on Ubuntu 22.04 passes `--config src-tauri/tauri.appimage-ort.conf.json`
 * so beforeBuildCommand becomes this script with `--appimage-ort`, and sets
 * ORT_LIB_PATH / ORT_PREFER_DYNAMIC_LINK / LD_LIBRARY_PATH for cargo + linuxdeploy.
 *
 * pyke's static ONNX Runtime 1.28 needs glibc 2.38; the shared Microsoft build
 * only needs glibc 2.27 (AppImageHub runner is Ubuntu 22.04 / glibc 2.35).
 */
const { execFileSync, execSync } = require('node:child_process');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const ORT_VERSION = '1.28.0';
const ORT_SHA256 =
  'a3e1b79d7bb1bf09696ce675f49e4064e6c81f6202b8225624fff0e93f8d6407';

const repoRoot = path.resolve(__dirname, '..');
const stageRoot = path.join(repoRoot, 'src-tauri', 'onnxruntime-linux');
const libDir = path.join(stageRoot, 'lib');
const sharedObject = path.join(libDir, `libonnxruntime.so.${ORT_VERSION}`);

const appimageOrt =
  process.argv.includes('--appimage-ort') ||
  process.env.LIBRAGENT_APPIMAGE_ORT === '1';

function sha256File(filePath) {
  const hash = crypto.createHash('sha256');
  hash.update(fs.readFileSync(filePath));
  return hash.digest('hex');
}

function stageOnnxRuntime() {
  if (process.platform !== 'linux' || process.arch !== 'x64') {
    return;
  }

  const linkerName = path.join(libDir, 'libonnxruntime.so');
  const soname = path.join(libDir, 'libonnxruntime.so.1');
  if (
    fs.existsSync(sharedObject) &&
    fs.existsSync(linkerName) &&
    fs.existsSync(soname)
  ) {
    ensureLoaderPath();
    return;
  }

  fs.mkdirSync(stageRoot, { recursive: true });
  const tarball = path.join(
    stageRoot,
    `onnxruntime-linux-x64-${ORT_VERSION}.tgz`,
  );
  const url = `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-linux-x64-${ORT_VERSION}.tgz`;
  execFileSync('curl', ['-fsSL', '-o', tarball, url], { stdio: 'inherit' });

  const digest = sha256File(tarball);
  if (digest !== ORT_SHA256) {
    fs.rmSync(tarball, { force: true });
    throw new Error(
      `ONNX Runtime ${ORT_VERSION} checksum mismatch: got ${digest}, expected ${ORT_SHA256}`,
    );
  }

  const extractDir = path.join(stageRoot, 'extract');
  fs.rmSync(extractDir, { recursive: true, force: true });
  fs.mkdirSync(extractDir, { recursive: true });
  execFileSync('tar', ['-xzf', tarball, '-C', extractDir], { stdio: 'inherit' });

  const extractedLib = path.join(
    extractDir,
    `onnxruntime-linux-x64-${ORT_VERSION}`,
    'lib',
  );
  fs.rmSync(libDir, { recursive: true, force: true });
  fs.cpSync(extractedLib, libDir, { recursive: true });
  fs.rmSync(extractDir, { recursive: true, force: true });
  fs.rmSync(tarball, { force: true });
  // cp follows the archive's symlinks into absolute paths under the extract
  // directory, which is then removed. Recreate relative SONAME links.
  fs.rmSync(linkerName, { force: true });
  fs.rmSync(soname, { force: true });
  fs.symlinkSync(`libonnxruntime.so.${ORT_VERSION}`, soname);
  fs.symlinkSync('libonnxruntime.so.1', linkerName);

  if (!fs.existsSync(sharedObject) || !fs.existsSync(linkerName)) {
    throw new Error(`ONNX Runtime library was not staged in ${libDir}`);
  }

  ensureLoaderPath();
}

/**
 * linuxdeploy resolves DT_NEEDED via the dynamic loader. CI must set
 * LD_LIBRARY_PATH (and ORT_LIB_PATH) before `tauri build` — no sudo/ldconfig.
 */
function ensureLoaderPath() {
  const ldPath = process.env.LD_LIBRARY_PATH || '';
  const parts = ldPath.split(path.delimiter).filter(Boolean);
  if (parts.includes(libDir)) {
    console.log(
      `ONNX Runtime staged at ${libDir}; LD_LIBRARY_PATH already includes it.`,
    );
    return;
  }

  const hint = `ONNX Runtime staged at ${libDir}. Export LD_LIBRARY_PATH (and ORT_LIB_PATH) to that directory before bundling so linuxdeploy can resolve libonnxruntime.so.1.`;
  if (process.env.ORT_PREFER_DYNAMIC_LINK === '1') {
    throw new Error(hint);
  }
  console.warn(hint);
}

if (!process.argv.includes('--onnx-only')) {
  execSync('pnpm build', { cwd: repoRoot, stdio: 'inherit' });
}

if (appimageOrt) {
  stageOnnxRuntime();
} else {
  console.log(
    'Skipping AppImage ONNX Runtime staging (pass --appimage-ort for Ubuntu 22.04 / AppImageHub builds).',
  );
}
