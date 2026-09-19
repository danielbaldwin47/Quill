#!/usr/bin/env python3
"""Bundle every board into one self-contained HTML page, for publishing as an Artifact.

`python3 bundle.py` reads canvas.json and the .dc.html boards beside it and writes
quill-settings-window.html: the boards as srcdoc iframes at their canvas positions,
on a surface that pans by drag and zooms by wheel. No network, no support.js.
"""
import html
import json
import pathlib
import re

HERE = pathlib.Path(__file__).parent
boards = json.loads((HERE / "canvas.json").read_text())["artboards"]

LABEL = 34  # px above each board for its title


def srcdoc(path):
    text = (HERE / path).read_text()
    body = re.search(r"<x-dc>(.*)</x-dc>", text, re.S).group(1)
    body = re.sub(r"<link[^>]*>", "", body)
    body = body.replace("<helmet>", "").replace("</helmet>", "")
    return "<!doctype html><meta charset='utf-8'>" + body


frames = []
for b in boards:
    frames.append(
        f"<div class='b' style='left:{b['x']}px;top:{b['y']}px;width:{b['w']}px'>"
        f"<div class='t'>{html.escape(b['title'])}</div>"
        f"<iframe loading='lazy' style='width:{b['w']}px;height:{b['h']}px' "
        f"srcdoc=\"{html.escape(srcdoc(b['file']), quote=True)}\"></iframe></div>"
    )

W = max(b["x"] + b["w"] for b in boards)
H = max(b["y"] + b["h"] for b in boards) + LABEL

page = f"""<!doctype html>
<html><head><meta charset="utf-8">
<title>Quill Settings window shape: round 1</title>
<style>
  html, body {{ margin: 0; height: 100%; overflow: hidden; background: #d9dadb;
    font-family: 'Adwaita Sans', Inter, 'Noto Sans', sans-serif; }}
  #view {{ position: absolute; inset: 0; cursor: grab; }}
  #view.drag {{ cursor: grabbing; }}
  #view.drag iframe {{ pointer-events: none; }}
  #canvas {{ position: absolute; transform-origin: 0 0; width: {W}px; height: {H}px; }}
  .b {{ position: absolute; }}
  .t {{ height: {LABEL}px; line-height: {LABEL}px; font-size: 15px; font-weight: 600; color: #3a3c3b;
    white-space: nowrap; }}
  iframe {{ border: 0; display: block; background: #fff; box-shadow: 0 2px 14px rgba(0,0,0,.18); }}
  #bar {{ position: fixed; left: 50%; bottom: 14px; transform: translateX(-50%); background: #1f2120;
    color: #f2f2f2; border-radius: 999px; padding: 6px 8px; font-size: 13px; display: flex; gap: 4px;
    box-shadow: 0 4px 18px rgba(0,0,0,.35); }}
  #bar button {{ background: none; border: 0; color: inherit; font: inherit; padding: 4px 10px;
    border-radius: 999px; cursor: pointer; }}
  #bar button:hover {{ background: #3a3c3b; }}
</style></head>
<body>
<div id="view"><div id="canvas">{''.join(frames)}</div></div>
<div id="bar"></div>
<script>
const view = document.getElementById('view'), canvas = document.getElementById('canvas');
let s = 0.5, x = 40, y = 40;
const apply = () => canvas.style.transform = `translate(${{x}}px,${{y}}px) scale(${{s}})`;
const rows = {json.dumps([[r, min(b['y'] for b in boards if b['title'].startswith(r))] for r in dict.fromkeys(b['title'].split(' ')[0] for b in boards)])};
const bar = document.getElementById('bar');
for (const [name, top] of rows) {{
  const btn = document.createElement('button'); btn.textContent = name;
  btn.onclick = () => {{ s = 0.6; x = 40; y = 60 - top * s; apply(); }};
  bar.appendChild(btn);
}}
const fit = document.createElement('button'); fit.textContent = 'All';
fit.onclick = () => {{ s = Math.min(innerWidth / {W + 80}, innerHeight / {H + 80}); x = 40 * s; y = 40 * s; apply(); }};
bar.appendChild(fit);
view.addEventListener('wheel', e => {{
  e.preventDefault();
  if (!e.shiftKey) {{
    const k = Math.exp(-e.deltaY * 0.0015), ns = Math.min(3, Math.max(0.08, s * k));
    x = e.clientX - (e.clientX - x) * ns / s; y = e.clientY - (e.clientY - y) * ns / s; s = ns;
  }} else {{ x -= e.deltaX; y -= e.deltaY; }}
  apply();
}}, {{ passive: false }});
let d = null;
view.addEventListener('pointerdown', e => {{ d = [e.clientX - x, e.clientY - y]; view.classList.add('drag'); view.setPointerCapture(e.pointerId); }});
view.addEventListener('pointermove', e => {{ if (d) {{ x = e.clientX - d[0]; y = e.clientY - d[1]; apply(); }} }});
view.addEventListener('pointerup', () => {{ d = null; view.classList.remove('drag'); }});
fit.onclick();
</script>
</body></html>
"""
out = HERE / "quill-settings-window.html"
out.write_text(page)
print(out, len(page) // 1024, "KB", len(boards), "boards")
