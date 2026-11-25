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
// Official Microsoft.ML.OnnxRuntime NuGet package (published 2026-07-25).
// GitHub release downloads for microsoft/onnxruntime return 404 since 2026-11-17.
const ORT_NUPKG_URL = `https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime/${ORT_VERSION}/microsoft.ml.onnxruntime.${ORT_VERSION}.nupkg`;
const ORT_NUPKG_SHA512 =
  'd97d83d031fa744cd67dab61e88deecc8b6c3c11b1a98951b0fcf73852bb4bd4df935d0c71674a19e80d8d3f0a98e02d9129e8df24c5f5ea38de60ef9fdb1a97';

const repoRoot = path.resolve(__dirname, '..');
const stageRoot = path.join(repoRoot, 'src-tauri', 'onnxruntime-linux');
const libDir = path.join(stageRoot, 'lib');
const sharedObject = path.join(libDir, `libonnxruntime.so.${ORT_VERSION}`);

const appimageOrt =
  process.argv.includes('--appimage-ort') ||
  process.env.LIBRAGENT_APPIMAGE_ORT === '1';

function sha512File(filePath) {
  const hash = crypto.createHash('sha512');
  hash.update(fs.readFileSync(filePath));
  return hash.digest('hex');
}

/** `fs.rmSync({ force: true })` silently skips dangling symlinks on Node 24. */
function removeLink(filePath) {
  try {
    fs.lstatSync(filePath);
  } catch {
    return;
  }
  fs.unlinkSync(filePath);
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
  const nupkg = path.join(
    stageRoot,
    `microsoft.ml.onnxruntime.${ORT_VERSION}.nupkg`,
  );
  execFileSync('curl', ['-fsSL', '-o', nupkg, ORT_NUPKG_URL], {
    stdio: 'inherit',
  });

  const digest = sha512File(nupkg);
  if (digest !== ORT_NUPKG_SHA512) {
    fs.rmSync(nupkg, { force: true });
    throw new Error(
      `ONNX Runtime ${ORT_VERSION} NuGet checksum mismatch: got ${digest}, expected ${ORT_NUPKG_SHA512}`,
    );
  }

  // The nupkg ships libonnxruntime.so (SONAME libonnxruntime.so.1) and
  // libonnxruntime_providers_shared.so under runtimes/linux-x64/native.
  fs.rmSync(libDir, { recursive: true, force: true });
  fs.mkdirSync(libDir, { recursive: true });
  execFileSync(
    'unzip',
    ['-q', '-o', '-j', nupkg, 'runtimes/linux-x64/native/*', '-d', libDir],
    { stdio: 'inherit' },
  );
  fs.rmSync(nupkg, { force: true });

  // Match the layout of the former GitHub tgz: versioned file plus SONAME and
  // linker-name symlinks.
  fs.renameSync(linkerName, sharedObject);
  removeLink(soname);
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
