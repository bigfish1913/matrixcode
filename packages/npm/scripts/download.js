#!/usr/bin/env node

const https = require('https');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const VERSION = require('./package.json').version;
const BIN_DIR = path.join(__dirname, 'bin');
const BIN_PATH = path.join(BIN_DIR, 'matrixcode');

// Determine platform and arch
const platform = process.platform;
const arch = process.arch;

// Map to release binary names
const binaryName = {
  win32: { x64: 'matrixcode-windows-x64.exe', arm64: 'matrixcode-windows-arm64.exe' },
  darwin: { x64: 'matrixcode-macos-x64', arm64: 'matrixcode-macos-arm64' },
  linux: { x64: 'matrixcode-linux-x64', arm64: 'matrixcode-linux-arm64' }
};

const binary = binaryName[platform]?.[arch];
if (!binary) {
  console.error(`Unsupported platform: ${platform}-${arch}`);
  process.exit(1);
}

// GitHub release URL
const downloadUrl = `https://github.com/bigfish1913/matrixcode/releases/download/v${VERSION}/${binary}`;

console.log(`Downloading matrixcode v${VERSION} for ${platform}-${arch}...`);

// Download function
function download(url, dest) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(dest);

    https.get(url, (response) => {
      if (response.statusCode === 302 || response.statusCode === 301) {
        // Follow redirect
        download(response.headers.location, dest).then(resolve).catch(reject);
        return;
      }

      if (response.statusCode !== 200) {
        reject(new Error(`Download failed: ${response.statusCode}`));
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
  });
}

// Main
async function main() {
  try {
    // Ensure bin directory exists
    if (!fs.existsSync(BIN_DIR)) {
      fs.mkdirSync(BIN_DIR, { recursive: true });
    }

    // Download binary
    await download(downloadUrl, BIN_PATH);

    // Make executable (Unix)
    if (platform !== 'win32') {
      fs.chmodSync(BIN_PATH, 0o755);
    }

    // On Windows, rename to .exe if needed
    if (platform === 'win32' && !BIN_PATH.endsWith('.exe')) {
      fs.renameSync(BIN_PATH, BIN_PATH + '.exe');
    }

    console.log('matrixcode installed successfully!');
    console.log('Run: matrixcode --help');
  } catch (err) {
    console.error('Installation failed:', err.message);
    console.error('\nYou can also install via cargo:');
    console.error('  cargo install matrixcode');
    process.exit(1);
  }
}

main();