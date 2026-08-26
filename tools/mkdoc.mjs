// Build the latency benchmark corpus: a real ~10,000-word Markdown document.
// Source: Alice's Adventures in Wonderland, Lewis Carroll (Project Gutenberg #11, public domain).
// The Gutenberg text already marks emphasis as _italic_; verse is indented. We unwrap the
// hard-wrapped paragraphs into one logical line per paragraph (how a document looks in iA Writer /
// Quill, and the harder case for a line-diffing renderer: long soft-wrapping lines).
//   node tools/mkdoc.mjs <alice.txt> <out.md> [targetWords=10000]
import fs from 'node:fs';
const [src, out, targetArg] = process.argv.slice(2);
const target = +(targetArg || 10000);
const raw = fs.readFileSync(src, 'utf8').replace(/\r\n/g, '\n');
const body = raw.slice(raw.indexOf('CHAPTER I.\n'), raw.indexOf('*** END OF THE PROJECT GUTENBERG'));
const ROMAN = { I: 1, II: 2, III: 3, IV: 4, V: 5, VI: 6, VII: 7, VIII: 8, IX: 9, X: 10, XI: 11, XII: 12 };
const words = (t) => (t.match(/[\p{L}\p{N}'’]+/gu) || []).length;

const blocks = body.split(/\n\s*\n/).map(b => b.replace(/\n+$/, ''));
const outLines = [
  '# Alice’s Adventures in Wonderland',
  '',
  'by Lewis Carroll — *working draft, marked up for the copy-edit pass*',
  '',
  '## Editor’s note',
  '',
  'This file is the [Project Gutenberg text](https://www.gutenberg.org/ebooks/11) reflowed one paragraph per line, with the original italics kept as Markdown emphasis. Before it goes to the typesetter:',
  '',
  '- [x] Reflow paragraphs to one logical line',
  '- [x] Keep Carroll’s italics (`_very_`, `_took a watch out of its waistcoat-pocket_`)',
  '- [ ] Check the verse indentation against the 1897 edition',
  '- [ ] Decide on **spaced en dashes** vs. em dashes throughout',
  '',
  '> “What is the use of a book,” thought Alice, “without pictures or conversations?”',
  '',
  'Build the reading copy with:',
  '',
  '```sh',
  'quill export alice.md --template classic --measure 66ch > alice.html',
  '```',
  '',
];
let n = words(outLines.join('\n'));
let chapter = 0, pendingTitle = null;
for (const b of blocks) {
  if (n >= target) break;
  const t = b.trim();
  if (!t) continue;
  let m;
  if ((m = /^CHAPTER ([IVX]+)\.\s*\n?([\s\S]*)$/.exec(t))) {
    chapter = ROMAN[m[1]];
    const title = m[2].trim().replace(/\n/g, ' ');
    if (title) { outLines.push('## ' + chapter + '. ' + title, ''); n += words(title); } else pendingTitle = true;
    continue;
  }
  if (pendingTitle) { outLines.push('## ' + chapter + '. ' + t.replace(/\n/g, ' '), ''); pendingTitle = false; n += words(t); continue; }
  if (/^\[Illustration\]$/.test(t)) continue;
  if (/^[*\s]+$/.test(t)) { if (outLines[outLines.length - 2] !== '* * *') outLines.push('* * *', ''); continue; }   // Carroll's scene break
  const indented = /^ {4,}/.test(b.split('\n')[0]);
  if (indented) {                       // verse / letter — keep the line breaks, quote it
    for (const l of b.split('\n')) outLines.push('> ' + l.trim());
    outLines.push('');
  } else {
    outLines.push(t.split('\n').map(s => s.trim()).join(' '));
    outLines.push('');
  }
  n += words(t);
}
outLines.push('---', '', '*End of the working draft. ' + n + ' words at the last count.*', '');
const doc = outLines.join('\n');
fs.writeFileSync(out, doc);
console.log(JSON.stringify({ out, words: words(doc), chars: doc.length, lines: doc.split('\n').length, paragraphs: doc.split('\n').filter(l => l.trim()).length }));
