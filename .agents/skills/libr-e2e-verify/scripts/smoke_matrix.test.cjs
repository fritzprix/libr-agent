const { test } = require('node:test');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const script = path.join(__dirname, 'smoke_matrix.cjs');

function userProfileDir() {
  const home = os.homedir();
  let base;
  if (process.platform === 'win32') {
    base = process.env.APPDATA || path.join(home, 'AppData', 'Roaming');
  } else if (process.platform === 'darwin') {
    base = path.join(home, 'Library', 'Application Support');
  } else {
    base = process.env.XDG_DATA_HOME || path.join(home, '.local', 'share');
  }
  return path.join(base, 'com.fritzprix.libragent');
}

function run(args) {
  return spawnSync(process.execPath, [script, ...args], {
    encoding: 'utf8',
    env: { ...process.env, LIBRAGENT_DATA_DIR: '' },
  });
}

test('refuses a user profile data dir', () => {
  const result = run(['--assistant', 'x', '--data-dir', userProfileDir()]);
  assert.equal(result.status, 2);
  assert.match(result.stderr, /user profile/);
});

test('refuses a user profile when only the path case differs', () => {
  const mixed = userProfileDir().replace(/com\.fritzprix\.libragent/i, 'Com.FritzPrix.LibrAgent');
  const result = run(['--assistant', 'x', '--data-dir', mixed]);
  if (process.platform === 'win32') {
    assert.equal(result.status, 2);
    assert.match(result.stderr, /user profile/);
  } else {
    assert.notEqual(result.status, 0);
  }
});

test('refuses an isolated dir that has no http_port file', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'libr-smoke-'));
  const result = run([
    '--assistant',
    'x',
    '--data-dir',
    dir,
    '--base-url',
    'http://127.0.0.1:3030',
  ]);
  assert.equal(result.status, 2);
  assert.match(result.stderr, /http_port/);
});

test('refuses a base-url whose port is not the isolated http_port', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'libr-smoke-'));
  fs.writeFileSync(path.join(dir, 'http_port'), '3999');
  const result = run([
    '--assistant',
    'x',
    '--data-dir',
    dir,
    '--base-url',
    'http://127.0.0.1:3030',
  ]);
  assert.equal(result.status, 2);
  assert.match(result.stderr, /does not match/);
  assert.equal(result.stdout, '');
});

test('refuses a base-url that is not loopback even when the port matches', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'libr-smoke-'));
  fs.writeFileSync(path.join(dir, 'http_port'), '3999');
  const result = run([
    '--assistant',
    'x',
    '--data-dir',
    dir,
    '--base-url',
    'http://example.com:3999',
  ]);
  assert.equal(result.status, 2);
  assert.match(result.stderr, /loopback/);
  assert.equal(result.stdout, '');
});
