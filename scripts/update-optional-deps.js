#!/usr/bin/env node
const fs = require('fs');
const path = require('path');

const newVersion = process.argv[2];
if (!newVersion) {
  console.error('Usage: node ./scripts/update-optional-deps.js <version>');
  process.exit(1);
}

const pkgPath = path.join(__dirname, '..', 'npm', 'agentsync', 'package.json');
if (!fs.existsSync(pkgPath)) {
  console.error('package.json not found at', pkgPath);
  process.exit(1);
}

const raw = fs.readFileSync(pkgPath, 'utf8');
let pkg;
try {
  pkg = JSON.parse(raw);
} catch (err) {
  console.error('Failed to parse package.json:', err.message);
  process.exit(1);
}

if (!pkg.optionalDependencies || typeof pkg.optionalDependencies !== 'object') {
  pkg.optionalDependencies = {};
}

Object.keys(pkg.optionalDependencies).forEach((k) => {
  pkg.optionalDependencies[k] = newVersion;
});

fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n', 'utf8');
console.log('Updated optionalDependencies in', pkgPath, 'to', newVersion);
