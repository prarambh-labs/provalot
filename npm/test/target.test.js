'use strict';
const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { target, assetName, assetUrl, expectedDigest, sha256File, verify } = require('../install.js');

test('maps platforms to release targets', () => {
  assert.strictEqual(target('darwin', 'arm64'), 'aarch64-apple-darwin');
  assert.strictEqual(target('darwin', 'x64'), 'x86_64-apple-darwin');
  assert.strictEqual(target('linux', 'x64'), 'x86_64-unknown-linux-musl');
  assert.strictEqual(target('linux', 'arm64'), 'aarch64-unknown-linux-musl');
  assert.strictEqual(target('win32', 'x64'), 'x86_64-pc-windows-msvc');
  assert.throws(() => target('sunos', 'x64'), /no prebuilt binary/);
});

test('asset names and urls pin the package version', () => {
  const { version } = require('../package.json');
  assert.strictEqual(assetName('x86_64-pc-windows-msvc'), 'provalot-x86_64-pc-windows-msvc.zip');
  assert.strictEqual(assetName('aarch64-apple-darwin'), 'provalot-aarch64-apple-darwin.tar.gz');
  delete process.env.PROVALOT_BINARY_URL;
  assert.ok(assetUrl('aarch64-apple-darwin').includes(`/v${version}/provalot-aarch64-apple-darwin.tar.gz`));
});

const HEX = 'a'.repeat(64);

test('expectedDigest fails closed without a recorded checksum', () => {
  assert.throws(() => expectedDigest('provalot-x.tar.gz', {}, {}), /no checksum recorded/);
  assert.strictEqual(expectedDigest('provalot-x.tar.gz', {}, { 'provalot-x.tar.gz': HEX.toUpperCase() }), HEX);
  assert.throws(() => expectedDigest('provalot-x.tar.gz', {}, { 'provalot-x.tar.gz': 'nothex' }), /no checksum recorded/);
});

test('a URL override must carry its own digest', () => {
  const url = { PROVALOT_BINARY_URL: 'https://example.invalid/x.tar.gz' };
  assert.throws(() => expectedDigest('provalot-x.tar.gz', url, { 'provalot-x.tar.gz': HEX }), /PROVALOT_BINARY_SHA256/);
  assert.throws(() => expectedDigest('provalot-x.tar.gz', { ...url, PROVALOT_BINARY_SHA256: 'short' }, {}), /PROVALOT_BINARY_SHA256/);
  assert.strictEqual(expectedDigest('provalot-x.tar.gz', { ...url, PROVALOT_BINARY_SHA256: HEX.toUpperCase() }, {}), HEX);
});

test('verify accepts a matching file and deletes a mismatching one', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'provalot-npm-'));
  const file = path.join(dir, 'asset.bin');
  fs.writeFileSync(file, 'hello');
  const good = sha256File(file);
  assert.strictEqual(good, '2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824');
  assert.doesNotThrow(() => verify(file, good));
  assert.ok(fs.existsSync(file));
  assert.throws(() => verify(file, HEX), /checksum mismatch/);
  assert.ok(!fs.existsSync(file), 'mismatching download is removed');
  fs.rmSync(dir, { recursive: true, force: true });
});
