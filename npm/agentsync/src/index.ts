#!/usr/bin/env node

import { spawnSync } from "child_process";
import { existsSync } from "fs";
import { join } from "path";

/**
 * Supported platforms and their npm package mappings
 */
const PLATFORMS: Record<string, string> = {
	"darwin-x64": "@dallay/agentsync-darwin-x64",
	"darwin-arm64": "@dallay/agentsync-darwin-arm64",
	"linux-x64": "@dallay/agentsync-linux-x64",
	"linux-arm64": "@dallay/agentsync-linux-arm64",
	"win32-x64": "@dallay/agentsync-windows-x64",
	"win32-arm64": "@dallay/agentsync-windows-arm64",
	"cygwin-x64": "@dallay/agentsync-windows-x64",
	"cygwin-arm64": "@dallay/agentsync-windows-arm64",
};

/**
 * Returns the executable path which is located inside `node_modules`
 * The naming convention is agentsync-${os}-${arch}
 * If the platform is `win32` or `cygwin`, executable will include a `.exe` extension.
 *
 * @see https://nodejs.org/api/os.html#osarch
 * @see https://nodejs.org/api/os.html#osplatform
 * @example "/path/to/node_modules/agentsync-darwin-arm64/bin/agentsync"
 */
function getExePath(): string {
	const platform = process.platform;
	const arch = process.arch;

	const platformKey = `${platform}-${arch}`;
	const packageName = PLATFORMS[platformKey];

	if (!packageName) {
		const supportedPlatforms = Object.keys(PLATFORMS)
			.map((p) => `  - ${p}`)
			.join("\n");
		throw new Error(
			`Unsupported platform: ${platformKey}\n\nSupported platforms:\n${supportedPlatforms}\n\nPlease open an issue at https://github.com/dallay/agentsync/issues`,
		);
	}

	// Determine binary name (with .exe on Windows and Cygwin)
	const binaryName =
		platform === "win32" || platform === "cygwin"
			? "agentsync.exe"
			: "agentsync";

	// Try to resolve the binary from the platform-specific package
	let binaryPath: string;

	try {
		// This works when the package is installed in node_modules
		const packagePath = require.resolve(`${packageName}/package.json`);
		binaryPath = join(packagePath, "..", "bin", binaryName);
	} catch {
		// Fallback: try to find it relative to this package
		binaryPath = join(
			__dirname,
			"..",
			"node_modules",
			packageName,
			"bin",
			binaryName,
		);
	}

	if (!existsSync(binaryPath)) {
		throw new Error(
			`Could not find agentsync binary at: ${binaryPath}\n\n` +
				`This usually means the platform-specific package (${packageName}) was not installed.\n` +
				`Try reinstalling with: npm install @dallay/agentsync\n\n` +
				`If the problem persists, please open an issue at https://github.com/dallay/agentsync/issues`,
		);
	}

	return binaryPath;
}

/**
 * Runs the agentsync binary with the provided arguments
 */
function run(): void {
	let binaryPath: string;

	try {
		binaryPath = getExePath();
	} catch (error) {
		console.error(`Error: ${(error as Error).message}`);
		process.exit(1);
	}

	// Pass all arguments to the binary (skip node and script path)
	const args = process.argv.slice(2);

	const result = spawnSync(binaryPath, args, {
		stdio: "inherit",
		env: process.env,
	});

	// Handle spawn errors
	if (result.error) {
		console.error(`Failed to execute agentsync: ${result.error.message}`);
		process.exit(1);
	}

	// Exit with the same code as the binary
	process.exit(result.status ?? 1);
}

run();
