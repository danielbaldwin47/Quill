// Make a JPEG data-URI thumbnail of a PNG using headless Chromium's canvas. node tools/thumb.mjs in.png [width=560] -> prints data URI
import { chromium } from 'playwright-core'; import fs from 'node:fs';
export async function thumb(file, width = 560, browser) {
  const own = !browser; if (own) browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
  const p = await browser.newPage();
  const data = 'data:image/png;base64,' + fs.readFileSync(file).toString('base64');
  const uri = await p.evaluate(async ([src, w]) => { const img = new Image(); img.src = src; await img.decode(); const c = document.createElement('canvas'); const s = w / img.width; c.width = w; c.height = Math.round(img.height * s); c.getContext('2d').drawImage(img, 0, 0, c.width, c.height); return c.toDataURL('image/jpeg', 0.82); }, [data, width]);
  await p.close(); if (own) await browser.close(); return uri;
}
if (process.argv[1] && process.argv[1].endsWith('thumb.mjs') && process.argv[2]) console.log(await thumb(process.argv[2], +(process.argv[3] || 560)));
