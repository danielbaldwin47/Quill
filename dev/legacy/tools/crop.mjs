// Crop a region out of a reference image (PNG/WebP/JPG) to PNG. node dev/legacy/tools/crop.mjs in.img out.png x y w h [scale=1]
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const [inp, out, x, y, w, h, scale = '1'] = process.argv.slice(2);
const mime = inp.endsWith('.webp') ? 'image/webp' : inp.endsWith('.jpg') || inp.endsWith('.jpeg') ? 'image/jpeg' : 'image/png';
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true }); const p = await b.newPage();
const uri = await p.evaluate(async ([src, x, y, w, h, s]) => { const img = new Image(); img.src = src; await img.decode(); const c = document.createElement('canvas'); c.width = Math.round(w * s); c.height = Math.round(h * s); c.getContext('2d').drawImage(img, x, y, w, h, 0, 0, c.width, c.height); return c.toDataURL('image/png'); }, ['data:' + mime + ';base64,' + fs.readFileSync(inp).toString('base64'), +x, +y, +w, +h, +scale]);
fs.writeFileSync(out, Buffer.from(uri.split(',')[1], 'base64')); await b.close(); console.log('wrote', out, Math.round(+w * +scale) + 'x' + Math.round(+h * +scale));
