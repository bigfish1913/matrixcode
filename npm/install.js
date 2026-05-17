#!/usr/bin/env node

const { execSync, spawnSync } = require('child_process');
const fs = require('fs');
const path = require('path');
const os = require('os');
const https = require('https');

const BIN_DIR = path.join(__dirname, 'bin');
const PACKAGE_VERSION = require('./package.json').version;

// Platform mapping for GitHub releases (matches release.yml targets)
const PLATFORM_MAP = {
  'darwin-x64': { target: 'x86_64-apple-darwin', archive: 'tar.gz' },
  'darwin-arm64': { target: 'aarch64-apple-darwin', archive: 'tar.gz' },
  'linux-x64': { target: 'x86_64-unknown-linux-gnu', archive: 'tar.gz' },
  'linux-arm64': { target: 'aarch64-unknown-linux-gnu', archive: 'tar.gz' },
  'win32-x64': { target: 'x86_64-pc-windows-msvc', archive: 'zip' },
  'win32-arm64': { target: 'aarch64-pc-windows-msvc', archive: 'zip' },
};

function getPlatform() {
  const platform = os.platform();
  const arch = os.arch();
  const key = `${platform}-${arch}`;
  return PLATFORM_MAP[key] || null;
}

function checkCommand(cmd) {
  try {
    execSync(`${cmd} --version`, { stdio: 'ignore' });
    return true;
  } catch {
    return false;
  }
}

function isMatrixcodeInstalled() {
  try {
    const result = spawnSync('matrixcode', ['--version'], { stdio: 'ignore' });
    return result.status === 0;
  } catch {
    return false;
  }
}

function installViaCargo() {
  console.log('[matrixcode] Installing via cargo install matrixcode...');
  try {
    execSync('cargo install matrixcode', { stdio: 'inherit' });
    console.log('[matrixcode] ✓ Installed successfully via cargo');
    return true;
  } catch (error) {
    console.error('[matrixcode] ✗ Failed to install via cargo:', error.message);
    return false;
  }
}

function downloadFile(url, dest) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(dest);
    const request = (urlString) => {
      https.get(urlString, (response) => {
        if (response.statusCode === 302 || response.statusCode === 301) {
          // Follow redirect
          request(response.headers.location);
          return;
        }
        if (response.statusCode !== 200) {
          reject(new Error(`HTTP ${response.statusCode}`));
          return;
        }
        response.pipe(file);
        file.on('finish', () => {
          file.close();
          resolve();
        });
      }).on('error', (err) => {
        fs.unlink(dest, () => {});
        reject(err);
      });
    };
    request(url);
  });
}

async function downloadBinary() {
  const platformInfo = getPlatform();
  if (!platformInfo) {
    console.error(`[matrixcode] Unsupported platform: ${os.platform()}-${os.arch()}`);
    console.log('[matrixcode] Falling back to cargo install...');
    return installViaCargo();
  }

  const { target, archive } = platformInfo;
  const archiveName = `matrixcode-${target}.${archive}`;
  const downloadUrl = `https://github.com/bigfish1913/matrixcode/releases/download/v${PACKAGE_VERSION}/${archiveName}`;
  
  console.log(`[matrixcode] Downloading binary for ${target}...`);
  console.log(`[matrixcode] URL: ${downloadUrl}`);

  // Create bin directory
  if (!fs.existsSync(BIN_DIR)) {
    fs.mkdirSync(BIN_DIR, { recursive: true });
  }

  const archivePath = path.join(BIN_DIR, archiveName);
  const binaryName = os.platform() === 'win32' ? 'matrixcode.exe' : 'matrixcode';
  const targetPath = path.join(BIN_DIR, binaryName);

  try {
    await downloadFile(downloadUrl, archivePath);
    console.log('[matrixcode] ✓ Download complete, extracting...');
  } catch (error) {
    console.error(`[matrixcode] ✗ Failed to download: ${error.message}`);
    console.log('[matrixcode] Falling back to cargo install...');
    return installViaCargo();
  }

  // Extract archive
  try {
    if (archive === 'tar.gz') {
      // Use tar to extract (available on macOS and Linux)
      execSync(`tar -xzf "${archivePath}" -C "${BIN_DIR}"`, { stdio: 'inherit' });
    } else if (archive === 'zip') {
      // On Windows, use PowerShell
      execSync(`powershell -Command "Expand-Archive -Path '${archivePath}' -DestinationPath '${BIN_DIR}' -Force"`, { stdio: 'inherit' });
    }
    
    // Make binary executable (Unix)
    if (os.platform() !== 'win32') {
      fs.chmodSync(targetPath, 0o755);
    }
    
    // Clean up archive
    fs.unlinkSync(archivePath);
    
    console.log('[matrixcode] ✓ Binary installed successfully');
    return true;
  } catch (error) {
    console.error(`[matrixcode] ✗ Failed to extract: ${error.message}`);
    // Clean up
    try { fs.unlinkSync(archivePath); } catch {}
    console.log('[matrixcode] Falling back to cargo install...');
    return installViaCargo();
  }
}

function createWrapperScript() {
  const ext = os.platform() === 'win32' ? '.cmd' : '';
  const wrapperPath = path.join(BIN_DIR, `matrixcode${ext}`);
  
  // Make sure bin directory exists
  if (!fs.existsSync(BIN_DIR)) {
    fs.mkdirSync(BIN_DIR, { recursive: true });
  }

  if (os.platform() === 'win32') {
    // Windows batch script
    const content = `@echo off
matrixcode %*
`;
    fs.writeFileSync(wrapperPath, content);
  } else {
    // Unix shell script - just call the system-installed matrixcode
    const content = `#!/bin/sh
exec matrixcode "$@"
`;
    fs.writeFileSync(wrapperPath, content, { mode: 0o755 });
  }
  
  console.log('[matrixcode] ✓ Wrapper script created');
}

async function main() {
  console.log(`[matrixcode] v${PACKAGE_VERSION}`);
  console.log('');

  // Check if matrixcode is already installed
  if (isMatrixcodeInstalled()) {
    console.log('[matrixcode] ✓ matrixcode is already installed');
    createWrapperScript();
    return;
  }

  console.log('[matrixcode] matrixcode not found, installing...');
  console.log('');

  // Prefer cargo if available (more reliable)
  if (checkCommand('cargo')) {
    console.log('[matrixcode] Found cargo, installing via cargo...');
    if (installViaCargo()) {
      createWrapperScript();
      return;
    }
  }

  // Try downloading pre-built binary
  if (await downloadBinary()) {
    return;
  }

  // Final fallback: show manual instructions
  console.error('');
  console.error('[matrixcode] ✗ Automatic installation failed');
  console.error('');
  console.error('Please install manually using one of these methods:');
  console.error('');
  console.error('  # Option 1: Using cargo (recommended)');
  console.error('  cargo install matrixcode');
  console.error('');
  console.error('  # Option 2: Download from GitHub Releases');
  console.error('  https://github.com/bigfish1913/matrixcode/releases');
  console.error('');
  process.exit(1);
}

main().catch(err => {
  console.error('[matrixcode] Installation error:', err.message);
  process.exit(1);
});