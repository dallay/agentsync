#!/usr/bin/env node
const fs = require('fs');
const path = require('path');

const newVersion = process.argv[2];
if (!newVersion) {
  console.error('Usage: node ./scripts/update-versions.js <version>');
  process.exit(1);
}

// 1. Update npm/agentsync/package.json
const pkgPath = path.join(__dirname, '..', 'npm', 'agentsync', 'package.json');
if (fs.existsSync(pkgPath)) {
  const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
  pkg.version = newVersion;
  if (pkg.optionalDependencies) {
    Object.keys(pkg.optionalDependencies).forEach((k) => {
      if (k.startsWith('@dallay/agentsync-')) {
        pkg.optionalDependencies[k] = newVersion;
      }
    });
  }
  fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n', 'utf8');
  console.log('✅ Updated npm/agentsync/package.json to', newVersion);
}

// 2. Update Cargo.toml
const cargoPath = path.join(__dirname, '..', 'Cargo.toml');
if (fs.existsSync(cargoPath)) {
  let cargo = fs.readFileSync(cargoPath, 'utf8');
  cargo = cargo.replace(/^version = ".*"$/m, `version = "${newVersion}"`);
  fs.writeFileSync(cargoPath, cargo, 'utf8');
  console.log('✅ Updated Cargo.toml to', newVersion);
}
