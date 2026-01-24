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

// 1. Update Cargo.toml
const cargoPath = path.join(rootDir, "Cargo.toml");
if (fs.existsSync(cargoPath)) {
	const cargoContent = fs.readFileSync(cargoPath, "utf8");
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
		fs.writeFileSync(cargoPath, updatedCargo);
		console.log("✅ Updated Cargo.toml");
	}
}

// 2. Update root package.json
const rootPkgPath = path.join(rootDir, "package.json");
if (fs.existsSync(rootPkgPath)) {
	const rootPkg = JSON.parse(fs.readFileSync(rootPkgPath, "utf8"));
	rootPkg.version = nextVersion;
	fs.writeFileSync(rootPkgPath, JSON.stringify(rootPkg, null, 2) + "\n");
	console.log("✅ Updated root package.json");
}

// 3. Update npm/agentsync/package.json
const npmPkgPath = path.join(rootDir, "npm/agentsync/package.json");
const npmPkgDir = path.join(rootDir, "npm/agentsync");
if (fs.existsSync(npmPkgPath)) {
	const npmPkg = JSON.parse(fs.readFileSync(npmPkgPath, "utf8"));
	npmPkg.version = nextVersion;
	fs.writeFileSync(npmPkgPath, JSON.stringify(npmPkg, null, 2) + "\n");
	console.log("✅ Updated npm/agentsync/package.json");
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
