const fs = require('node:fs');
const path = require('node:path');
const { execSync } = require('node:child_process');

// Config
const ROOT_DIR = path.resolve(__dirname, '..');
const SOURCE_DOCS = path.join(ROOT_DIR, 'website/docs/src/content/docs');
const TARGET_DOCS = path.join(ROOT_DIR, 'docs');

function runCommand(command) {
  try {
    console.log(`> ${command}`);
    execSync(command, { stdio: 'inherit', cwd: ROOT_DIR });
  } catch (error) {
    console.error(`Command failed: ${command}`);
    process.exit(1);
  }
}

function setupGitHooks() {
  const gitDir = path.join(ROOT_DIR, '.git');
  if (fs.existsSync(gitDir)) {
    console.log('📦 Git repository detected. Setting up hooks and syncing agents...');
    runCommand('pnpm exec lefthook install');
    runCommand('pnpm run agents:sync');
  } else {
    console.log('⚠️ Not a git repository. Skipping git-specific setup.');
  }
}

function createSymlink() {
  console.log('🔗 Setting up documentation symlink...');

  if (!fs.existsSync(SOURCE_DOCS)) {
    console.error(`Error: Source directory not found: ${SOURCE_DOCS}`);
    process.exit(1);
  }

  if (fs.existsSync(TARGET_DOCS)) {
    const stats = fs.lstatSync(TARGET_DOCS);
    if (stats.isSymbolicLink()) {
      console.log('  Symlink already exists. Skipping.');
      return;
    }
    console.error(`Error: Target path exists and is not a symlink: ${TARGET_DOCS}`);
    process.exit(1);
  }

  // Windows requires 'junction' for directory symlinks to avoid admin prompt requirements
  // Unix ignores the 'type' argument.
  const type = process.platform === 'win32' ? 'junction' : 'dir';
  
  try {
    fs.symlinkSync(SOURCE_DOCS, TARGET_DOCS, type);
    console.log(`  ✅ Created symlink: ${TARGET_DOCS} -> ${SOURCE_DOCS}`);
  } catch (error) {
    console.error('  Failed to create symlink:', error.message);
    process.exit(1);
  }
}

// Main execution
console.log('🚀 Starting environment setup...');
setupGitHooks();
createSymlink();
console.log('✨ Setup complete.');
