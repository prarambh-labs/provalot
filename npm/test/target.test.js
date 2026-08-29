'use strict';
const test = require('node:test');
const assert = require('node:assert');
const { target, assetName, assetUrl } = require('../install.js');

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
