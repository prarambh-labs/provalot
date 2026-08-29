#!/usr/bin/env node
'use strict';
const fs = require('fs');
const path = require('path');
const https = require('https');
const { execFileSync } = require('child_process');
const { version } = require('./package.json');

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
  return process.env.PROVALOT_BINARY_URL || `https://github.com/vaishach0523-P1/provalot/releases/download/v${version}/${assetName(t)}`;
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
  await download(assetUrl(t), asset);
  if (asset.endsWith('.zip')) {
    execFileSync('powershell', ['-NoProfile', '-Command', `Expand-Archive -Force -Path "${asset}" -DestinationPath "${binDir}"`]);
  } else {
    execFileSync('tar', ['-xzf', asset, '-C', binDir]);
    fs.chmodSync(path.join(binDir, 'provalot'), 0o755);
  }
  fs.unlinkSync(asset);
}

module.exports = { target, assetName, assetUrl };

if (require.main === module) {
  main().catch((e) => {
    console.error(e.message);
    process.exit(1);
  });
}
