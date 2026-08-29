#!/usr/bin/env node
'use strict';
const path = require('path');
const { spawnSync } = require('child_process');
const exe = path.join(__dirname, process.platform === 'win32' ? 'provalot.exe' : 'provalot');
const r = spawnSync(exe, process.argv.slice(2), { stdio: 'inherit' });
if (r.error) {
  console.error(`provalot: binary missing at ${exe}. Run: npm rebuild provalot, or: cargo install provalot`);
  process.exit(1);
}
process.exit(r.status === null ? 1 : r.status);
