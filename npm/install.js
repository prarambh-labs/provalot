#!/usr/bin/env node
'use strict';
const fs = require('fs');
const path = require('path');
const https = require('https');
const crypto = require('crypto');
const { execFileSync } = require('child_process');
const { version } = require('./package.json');
const CHECKSUMS = require('./checksums.json');

const TARGETS = {
  'darwin-arm64': 'aarch64-apple-darwin',
  'darwin-x64': 'x86_64-apple-darwin',
  'linux-x64': 'x86_64-unknown-linux-musl',
  'linux-arm64': 'aarch64-unknown-linux-musl',
  'win32-x64': 'x86_64-pc-windows-msvc',
};

function target(platform = process.platform, arch = process.arch) {
  const key = `${platform}-${arch}`;
  const t = TARGETS[key];
  if (!t) throw new Error(`provalot: no prebuilt binary for ${key}; install with: cargo install provalot`);
  return t;
}

function assetName(t) {
  return t.includes('windows') ? `provalot-${t}.zip` : `provalot-${t}.tar.gz`;
}

function assetUrl(t) {
  return process.env.PROVALOT_BINARY_URL || `https://github.com/prarambh-labs/provalot/releases/download/v${version}/${assetName(t)}`;
}

/**
 * SHA-256 hex digest the downloaded asset must match. Fails closed: no recorded digest means no install.
 * A PROVALOT_BINARY_URL override must be paired with PROVALOT_BINARY_SHA256.
 */
function expectedDigest(name, env = process.env, checksums = CHECKSUMS) {
  if (env.PROVALOT_BINARY_URL) {
    const d = env.PROVALOT_BINARY_SHA256;
    if (!d || !/^[0-9a-fA-F]{64}$/.test(d)) {
      throw new Error('provalot: PROVALOT_BINARY_URL requires PROVALOT_BINARY_SHA256 (64 hex chars) so the download can be verified');
    }
    return d.toLowerCase();
  }
  const d = checksums[name];
  if (!d || !/^[0-9a-fA-F]{64}$/.test(d)) {
    throw new Error(`provalot: no checksum recorded for ${name} in this package; refusing to install an unverified binary. Install with: cargo install provalot`);
  }
  return d.toLowerCase();
}

function sha256File(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

/** Deletes the file and throws when its digest does not match. */
function verify(file, expected) {
  const got = sha256File(file);
  if (got !== expected) {
    fs.unlinkSync(file);
    throw new Error(`provalot: checksum mismatch for ${path.basename(file)}: expected ${expected}, got ${got}`);
  }
}

function download(url, dest, redirects = 0) {
  return new Promise((resolve, reject) => {
    https
      .get(url, { headers: { 'user-agent': 'provalot-npm' } }, (res) => {
        if ([301, 302, 303, 307, 308].includes(res.statusCode) && res.headers.location && redirects < 5) {
          res.resume();
          return resolve(download(res.headers.location, dest, redirects + 1));
        }
        if (res.statusCode !== 200) {
          res.resume();
          return reject(new Error(`provalot: download failed (${res.statusCode}) ${url}`));
        }
        const file = fs.createWriteStream(dest);
        res.pipe(file);
        file.on('finish', () => file.close(resolve));
        file.on('error', reject);
      })
      .on('error', reject);
  });
}

async function main() {
  const t = target();
  const binDir = path.join(__dirname, 'bin');
  fs.mkdirSync(binDir, { recursive: true });
  const asset = path.join(binDir, assetName(t));
  const expected = expectedDigest(assetName(t));
  await download(assetUrl(t), asset);
  verify(asset, expected);
  if (asset.endsWith('.zip')) {
    execFileSync('powershell', ['-NoProfile', '-Command', `Expand-Archive -Force -Path "${asset}" -DestinationPath "${binDir}"`]);
  } else {
    execFileSync('tar', ['-xzf', asset, '-C', binDir]);
    fs.chmodSync(path.join(binDir, 'provalot'), 0o755);
  }
  fs.unlinkSync(asset);
}

module.exports = { target, assetName, assetUrl, expectedDigest, sha256File, verify };

if (require.main === module) {
  main().catch((e) => {
    console.error(e.message);
    process.exit(1);
  });
}
