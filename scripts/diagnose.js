#!/usr/bin/env node
const { exec } = require('child_process');
const os = require('os');

function runCmd(cmd, label) {
  return new Promise((resolve) => {
    exec(cmd, { shell: true, maxBuffer: 1024 * 1024 * 10 }, (err, stdout, stderr) => {
      console.log(`\n--- ${label} ---`);
      if (err) {
        console.log(`(command: ${cmd}) returned error: ${err.message}`);
        if (stdout && stdout.trim()) console.log('STDOUT:\n' + stdout.trim());
        if (stderr && stderr.trim()) console.log('STDERR:\n' + stderr.trim());
        return resolve({ ok: false, err: err.message });
      }
      if (stdout && stdout.trim()) console.log(stdout.trim());
      if (stderr && stderr.trim()) console.log('STDERR:\n' + stderr.trim());
      resolve({ ok: true });
    });
  });
}

(async function main() {
  console.log('System diagnostic...');
  console.log('Platform:', process.platform);
  console.log('OS type:', os.type());
  console.log('OS release:', os.release());
  console.log('Arch:', os.arch());
  console.log('Node:', process.version);

  // Try to show npm version if available
  try {
    const npmVer = (await new Promise((res) => exec('npm --version', { shell: true }, (_, stdout) => res(stdout && stdout.trim())))) || 'unknown';
    console.log('npm:', npmVer);
  } catch (e) {
    console.log('npm: unknown');
  }

  // Platform-specific checks
  if (process.platform === 'linux') {
    await runCmd('ldd --version', 'ldd --version');
    await runCmd('which dpkg || true', 'dpkg presence check');
    await runCmd('dpkg -l | grep webkit || true', 'WebKit packages (dpkg)');
  } else if (process.platform === 'darwin') {
    await runCmd('uname -a', 'uname -a');
    // Homebrew presence and webkit package attempt
    await runCmd('which brew || true', 'brew presence check');
    await runCmd('brew list --versions webkit || true', 'WebKit packages (brew)');
  } else if (process.platform === 'win32') {
    // Windows: use ver and systeminfo (systeminfo may be slow/require privileges)
    await runCmd('ver', 'Windows ver');
    await runCmd('systeminfo | findstr /B /C:"OS Name" /C:"OS Version" || true', 'systeminfo (OS Name/Version)');
    // Check for PowerShell availability
    await runCmd('powershell -Command "Get-Host | Select-Object Version"', 'PowerShell version');
    // Check for WebKit-related packages is unreliable on Windows; skip
  } else {
    await runCmd('uname -a', 'uname -a');
  }

  console.log('\nDiagnostic complete.');
})();
