#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

/**
 * Syncs optionalDependencies with a target version.
 * Usage: node sync-optional-deps.js [version]
 */

const targetVersion = process.argv[2];
const packageJsonPath = path.join(process.cwd(), "package.json");

if (!fs.existsSync(packageJsonPath)) {
	console.error("❌ package.json not found in", process.cwd());
	process.exit(1);
}

const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));
const versionToUse = targetVersion || packageJson.version;

console.log(`🔄 Syncing optionalDependencies to version: ${versionToUse}`);

let changed = false;
if (packageJson.optionalDependencies) {
	for (const depName of Object.keys(packageJson.optionalDependencies)) {
		if (depName.startsWith("@dallay/agentsync-")) {
			if (packageJson.optionalDependencies[depName] !== versionToUse) {
				packageJson.optionalDependencies[depName] = versionToUse;
				console.log(`  ✓ Updated ${depName} → ${versionToUse}`);
				changed = true;
			}
		}
	}
}

if (changed) {
	fs.writeFileSync(
		packageJsonPath,
		JSON.stringify(packageJson, null, 2) + "\n",
	);
	console.log("✅ package.json updated successfully");
} else {
	console.log("ℹ️ No changes needed");
}
