#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

/**
 * Syncs optionalDependencies with a target version.
 * Usage: node sync-optional-deps.js [version]
 */

const targetVersion = process.argv[2];
const packageJsonPath = path.join(process.cwd(), "package.json");

let fd;
try {
	// Open for reading and writing ('r+')
	// This throws if the file doesn't exist, replacing the fs.existsSync check
	fd = fs.openSync(packageJsonPath, "r+");
} catch (error) {
	if (error.code === "ENOENT") {
		console.error("❌ package.json not found in", process.cwd());
		process.exit(1);
	}
	throw error;
}

try {
	const content = fs.readFileSync(fd, "utf8");
	const packageJson = JSON.parse(content);
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
		const newContent = JSON.stringify(packageJson, null, 2) + "\n";
		// Truncate file to 0 length to remove old content
		fs.ftruncateSync(fd, 0);
		// Write new content starting at position 0
		fs.writeSync(fd, newContent, 0, "utf8");
		console.log("✅ package.json updated successfully");
	} else {
		console.log("ℹ️ No changes needed");
	}
} finally {
	if (fd !== undefined) {
		fs.closeSync(fd);
	}
}
