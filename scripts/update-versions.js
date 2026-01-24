const fs = require("fs");
const path = require("path");
const { execFileSync } = require("child_process");

const SEMVER_RE = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;
const rawNextVersion = process.argv[2];
const nextVersion = rawNextVersion && rawNextVersion.trim();

if (!nextVersion || !SEMVER_RE.test(nextVersion)) {
	console.error(
		"Error: Invalid or missing version (expected SemVer, e.g., 1.2.3)",
	);
	process.exit(1);
}

console.log(`🚀 Updating project versions to ${nextVersion}...`);

const rootDir = path.resolve(__dirname, "..");
let hadErrors = false;

// 1. Update Cargo.toml (use file descriptor to avoid TOCTOU)
const cargoPath = path.join(rootDir, "Cargo.toml");
try {
	const fd = fs.openSync(cargoPath, "r+");
	try {
		const cargoContent = fs.readFileSync(fd, "utf8");
		let versionUpdated = false;

		// Robust regex to find version inside [package] or [workspace.package]
		const updatedCargo = cargoContent.replace(
			/(\[(?:workspace\.)?package\][\s\S]*?^\s*version\s*=\s*")([^"]*)(")/m,
			(match, prefix, oldVersion, suffix) => {
				versionUpdated = true;
				console.log(
					`  Found Cargo.toml version: ${oldVersion} inside package section`,
				);
				return `${prefix}${nextVersion}${suffix}`;
			},
		);

		if (!versionUpdated) {
			console.error(
				"❌ Could not find version line inside [package] or [workspace.package] section of Cargo.toml",
			);
			// Log the first 200 chars to debug
			console.log("--- Cargo.toml content start ---");
			console.log(cargoContent.substring(0, 200));
			console.log("--- End ---");
			hadErrors = true;
		} else {
			// Truncate and write using the same file descriptor to avoid races
			fs.ftruncateSync(fd, 0);
			fs.writeSync(fd, updatedCargo, 0, "utf8");
			fs.fsyncSync(fd);
			console.log("✅ Updated Cargo.toml");
		}
	} finally {
		try {
			fs.closeSync(fd);
		} catch (e) {
			/* best-effort close */
		}
	}
} catch (err) {
	if (err && err.code === "ENOENT") {
		// File doesn't exist; nothing to do
	} else if (err) {
		console.error("❌ Failed updating Cargo.toml:", err.message || err);
		hadErrors = true;
	}
}

// 2. Update root package.json (use file descriptor to avoid TOCTOU)
const rootPkgPath = path.join(rootDir, "package.json");
try {
	const fd = fs.openSync(rootPkgPath, "r+");
	try {
		const current = fs.readFileSync(fd, "utf8");
		const rootPkg = JSON.parse(current);
		rootPkg.version = nextVersion;
		const newContents = JSON.stringify(rootPkg, null, 2) + "\n";

		fs.ftruncateSync(fd, 0);
		fs.writeSync(fd, newContents, 0, "utf8");
		fs.fsyncSync(fd);
		console.log("✅ Updated root package.json");
	} finally {
		try {
			fs.closeSync(fd);
		} catch (e) {
			/* best-effort close */
		}
	}
} catch (err) {
	if (err && err.code === "ENOENT") {
		// File doesn't exist; skip
	} else if (err) {
		console.error("❌ Failed updating root package.json:", err.message || err);
		hadErrors = true;
	}
}

// 3. Update npm/agentsync/package.json (use file descriptor to avoid TOCTOU)
const npmPkgPath = path.join(rootDir, "npm/agentsync/package.json");
const npmPkgDir = path.join(rootDir, "npm/agentsync");
try {
	const fd = fs.openSync(npmPkgPath, "r+");
	try {
		const current = fs.readFileSync(fd, "utf8");
		const npmPkg = JSON.parse(current);
		npmPkg.version = nextVersion;
		const newContents = JSON.stringify(npmPkg, null, 2) + "\n";

		fs.ftruncateSync(fd, 0);
		fs.writeSync(fd, newContents, 0, "utf8");
		fs.fsyncSync(fd);
		console.log("✅ Updated npm/agentsync/package.json");
	} finally {
		try {
			fs.closeSync(fd);
		} catch (e) {
			/* best-effort close */
		}
	}
} catch (err) {
	if (err && err.code === "ENOENT") {
		// File doesn't exist; skip
	} else if (err) {
		console.error(
			"❌ Failed updating npm/agentsync/package.json:",
			err.message || err,
		);
		hadErrors = true;
	}
}

// 4. Run sync-optional-deps.js if it exists
const syncOptionalDepsPath = path.join(
	npmPkgDir,
	"scripts/sync-optional-deps.js",
);
if (fs.existsSync(syncOptionalDepsPath)) {
	console.log("🔄 Running sync-optional-deps.js...");
	try {
		// We run it from the npm/agentsync directory using execFileSync for safety
		execFileSync(
			process.execPath,
			["scripts/sync-optional-deps.js", nextVersion],
			{
				cwd: npmPkgDir,
				stdio: "inherit",
			},
		);
		console.log("✅ Updated optional dependencies");
	} catch (error) {
		console.error("❌ Error running sync-optional-deps.js:", error.message);
		hadErrors = true;
	}
}

if (hadErrors) {
	console.error("❌ Version update completed with errors.");
	process.exit(1);
}

console.log("🎉 All versions updated successfully!");
