// Render a few raw artboards to PNG for a look before the canvas is saved.
import { createRequire } from 'node:module';
import { readFileSync, existsSync } from 'node:fs';
const { chromium } = createRequire('/home/diggle/repos/quill/package.json')('playwright-core');
const dir = '/tmp/claude-1000/-home-diggle-repos-quill/d9740f10-5512-4477-8ea4-56eb488ecdac/scratchpad/pane';
const files = process.argv.slice(2);
const exe = ['/usr/bin/chromium', '/usr/bin/chromium-browser', '/usr/bin/google-chrome', '/usr/bin/google-chrome-stable'].find(existsSync);
const browser = await chromium.launch({ headless: true, executablePath: exe, args: ['--no-sandbox'] });
const page = await browser.newPage({ viewport: { width: 760, height: 640 }, deviceScaleFactor: 2 });
for (const f of files) {
  const html = readFileSync(`${dir}/${f}`, 'utf8').replace('<script src="./support.js"></script>', '');
  await page.setContent(html, { waitUntil: 'networkidle' });
  await page.evaluate(() => document.fonts.ready);
  await page.screenshot({ path: `${dir}/shot-${f.replace('.dc.html', '')}.png` });
  console.log('shot', f);
}
await browser.close();
