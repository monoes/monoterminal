#!/usr/bin/env node

/**
 * PWA Icon Generator for MONOTERMINAL
 * Generates required PWA icons from SVG sources using sharp
 *
 * Required icons per SRS §2.2:
 * - pwa-192x192.png
 * - pwa-512x512.png
 * - apple-touch-icon.png (180x180)
 *
 * Usage:
 *   npm run generate-icons
 *
 * Note: Requires sharp for SVG to PNG conversion
 */

import sharp from 'sharp';
import { readFileSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const PUBLIC_DIR = join(__dirname, '..', 'public');

async function generateIcon(svgPath, outputPath, size) {
  try {
    const svgBuffer = readFileSync(svgPath);
    await sharp(svgBuffer)
      .resize(size, size, {
        fit: 'contain',
        background: { r: 30, g: 30, b: 30, alpha: 1 }
      })
      .png()
      .toFile(outputPath);
    console.log(`✓ Generated ${outputPath} (${size}x${size})`);
    return true;
  } catch (error) {
    console.error(`✗ Failed to generate ${outputPath}:`, error.message);
    return false;
  }
}

async function main() {
  console.log('Generating PWA icons for MONOTERMINAL...\n');

  const icons = [
    {
      source: join(PUBLIC_DIR, 'pwa-192x192.svg'),
      output: join(PUBLIC_DIR, 'pwa-192x192.png'),
      size: 192
    },
    {
      source: join(PUBLIC_DIR, 'pwa-512x512.svg'),
      output: join(PUBLIC_DIR, 'pwa-512x512.png'),
      size: 512
    },
    {
      source: join(PUBLIC_DIR, 'apple-touch-icon.svg'),
      output: join(PUBLIC_DIR, 'apple-touch-icon.png'),
      size: 180
    },
  ];

  let successCount = 0;
  let failCount = 0;

  for (const icon of icons) {
    if (!existsSync(icon.source)) {
      console.error(`✗ Source file not found: ${icon.source}`);
      failCount++;
      continue;
    }

    const success = await generateIcon(icon.source, icon.output, icon.size);
    if (success) {
      successCount++;
    } else {
      failCount++;
    }
  }

  console.log(`\n${successCount} icons generated successfully!`);
  if (failCount > 0) {
    console.error(`${failCount} icons failed to generate.`);
    process.exit(1);
  }

  console.log('\nPWA icons ready for production!');
  console.log('To test PWA installation:');
  console.log('  1. npm run build');
  console.log('  2. npm run preview');
  console.log('  3. Open DevTools → Application → Manifest');
  console.log('  4. Check "Add to Home Screen" functionality');
}

main().catch(err => {
  console.error('Fatal error:', err);
  process.exit(1);
});
