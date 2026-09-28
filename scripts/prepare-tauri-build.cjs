/**
 * Frontend build, then (Linux x64 only) stage Microsoft's ONNX Runtime shared
 * library for the Tauri bundle.
 *
 * pyke's prebuilt static ONNX Runtime 1.28 needs glibc 2.38. AppImageHub runs
 * on Ubuntu 22.04 (glibc 2.35), so the release workflow links this shared
 * build instead. It only needs glibc 2.27. `beforeBuildCommand` runs this
 * before `cargo build`; the Ubuntu job sets ORT_LIB_PATH at this directory.
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
    registerWithDynamicLinker();
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

  registerWithDynamicLinker();
}

function registerWithDynamicLinker() {
  // linuxdeploy resolves DT_NEEDED via the system loader. Register this
  // directory so the AppImage bundle copies libonnxruntime.so.1.
  const conf = '/etc/ld.so.conf.d/libragent-onnxruntime.conf';
  try {
    execFileSync('sudo', ['tee', conf], {
      input: `${libDir}\n`,
      stdio: ['pipe', 'inherit', 'inherit'],
    });
    execFileSync('sudo', ['ldconfig'], { stdio: 'inherit' });
  } catch (error) {
    const message =
      'Could not register ONNX Runtime with ldconfig. AppImage bundling needs that library on the dynamic loader path.';
    if (process.env.ORT_PREFER_DYNAMIC_LINK === '1') {
      throw new Error(message, { cause: error });
    }
    console.warn(message);
  }
}

if (!process.argv.includes('--onnx-only')) {
  execSync('pnpm build', { cwd: repoRoot, stdio: 'inherit' });
}

stageOnnxRuntime();
