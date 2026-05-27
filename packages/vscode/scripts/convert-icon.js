/**
 * Convert SVG to PNG for VSCode extension icon
 */

const { Resvg } = require('@resvg/resvg-js');
const fs = require('fs');
const path = require('path');

const baseDir = path.join(__dirname, '..');
const svgPath = path.join(baseDir, 'resources', 'icon.svg');
const pngPath = path.join(baseDir, 'resources', 'icon.png');

// Read SVG file
const svgContent = fs.readFileSync(svgPath, 'utf-8');

// Configure Resvg options
const opts = {
  fitTo: {
    mode: 'width',
    value: 128,  // VSCode recommends 128x128
  },
};

// Create Resvg instance and render
const resvg = new Resvg(svgContent, opts);
const rendered = resvg.render();

// Convert to PNG buffer
const pngBuffer = rendered.asPng();

// Write PNG file
fs.writeFileSync(pngPath, pngBuffer);

console.log(`Successfully converted ${svgPath} to ${pngPath}`);
console.log(`PNG size: ${rendered.width}x${rendered.height}`);