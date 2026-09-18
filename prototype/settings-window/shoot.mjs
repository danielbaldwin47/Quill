// Render artboards to shots/<stem>.png at 2x: `node shoot.mjs` for every board in canvas.json,
// `node shoot.mjs A1ExportDark.dc.html ...` for some. Headless chromium only.
import { createRequire } from 'node:module';
import { readFileSync, existsSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
const { chromium } = createRequire('/home/diggle/repos/quill/package.json')('playwright-core');
const dir = dirname(fileURLToPath(import.meta.url));
const canvas = JSON.parse(readFileSync(join(dir, 'canvas.json'), 'utf8'));
const want = process.argv.slice(2);
const boards = canvas.artboards.filter((b) => want.length === 0 || want.includes(b.file));
mkdirSync(join(dir, 'shots'), { recursive: true });
const exe = ['/usr/bin/chromium', '/usr/bin/chromium-browser', '/usr/bin/google-chrome-stable'].find(existsSync);
const browser = await chromium.launch({ headless: true, executablePath: exe, args: ['--no-sandbox'] });
const page = await browser.newPage({ deviceScaleFactor: 2 });
for (const b of boards) {
  await page.setViewportSize({ width: b.w, height: b.h });
  const html = readFileSync(join(dir, b.file), 'utf8').replace('<script src="./support.js"></script>', '');
  await page.setContent(html, { waitUntil: 'networkidle' });
  await page.evaluate(() => document.fonts.ready);
  await page.screenshot({ path: join(dir, 'shots', b.file.replace('.dc.html', '.png')) });
  console.log('shot', b.file);
}
await browser.close();
