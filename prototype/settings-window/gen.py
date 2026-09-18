#!/usr/bin/env python3
"""Generate the Settings window direction artboards (.dc.html) and canvas.json (#464).

Rows are exactly what #463 leaves in Settings. Two skins draw the same widgets:
STOCK is GTK4's default theme as sampled from today/settings-{light,dark}-1-top.png,
QUILL is the app's own palette (quill-engine/src/theme.rs) and menu styling
(quill/src/chrome.rs, menu_ink and the chrome-menu constants). 1 CSS px = 1 logical px.

Every style is inline, and every font stack inside a style attribute is single-quoted:
a double-quoted font name ends the attribute early and silently drops the rest.
"""
import json, os, html

OUT = os.path.dirname(os.path.abspath(__file__))
SANS = "'Adwaita Sans', Inter, 'Noto Sans', sans-serif"
MONO = "'iA Writer Mono S', 'JetBrains Mono', ui-monospace, monospace"
FONTS = ('<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700'
         '&amp;family=JetBrains+Mono:wght@400;700&amp;display=swap">')

# ---------- skins ----------
STOCK = {
 "light": dict(name="stock", fs=14.67, hint_fs=12.5, ink="#2e3436", dim="#8b8e8f", strong="#2e3436", bg="#f7f7f7",
               rule="#cdc7c2", row=38, sw=(50, 26, 22), sw_off="#e1dedb", sw_on="#3584e4", knob="#f8f7f7",
               knob_bd="#cdc7c2", btn_bg="#fbfafa", btn_bd="#cdc7c2", btn_h=34, btn_pad=16, radius=5,
               field="#ffffff", accent="#3584e4", check_bd="#cdc7c2", list_bg="#ffffff", danger="#c01c28",
               track="#e1dedb", head_fs=14.67, head_w=700, head_caps=False, sub_w=400),
 "dark":  dict(name="stock", fs=14.67, hint_fs=12.5, ink="#eeeeec", dim="#8e8e8d", strong="#eeeeec", bg="#1a1a1a",
               rule="#0e0e0e", row=38, sw=(50, 26, 22), sw_off="#282828", sw_on="#15539e", knob="#383838",
               knob_bd="#1b1b1b", btn_bg="#383838", btn_bd="#1b1b1b", btn_h=34, btn_pad=16, radius=5,
               field="#2d2d2d", accent="#15539e", check_bd="#1b1b1b", list_bg="#2d2d2d", danger="#ff7b63",
               track="#282828", head_fs=14.67, head_w=700, head_caps=False, sub_w=400),
}
QUILL = {
 "light": dict(name="quill", fs=13, hint_fs=11.5, ink="#191919", dim="#8c8c8c", strong="#4a4a4a", bg="#f7f7f7",
               rule="rgba(0,0,0,0.10)", row=32, sw=(32, 18, 14), sw_off="#d2d2d2", sw_on="#0a94d6", knob="#ffffff",
               knob_bd="rgba(0,0,0,0.08)", btn_bg="#fcfcfc", btn_bd="rgba(0,0,0,0.16)", btn_h=24, btn_pad=10, radius=5,
               field="#fcfcfc", accent="#0a94d6", check_bd="rgba(0,0,0,0.28)", list_bg="#fcfcfc", danger="#c4483c",
               track="#dcdcdc", head_fs=10.5, head_w=600, head_caps=True, sub_w=500,
               side="#eaebeb", pill="#d8d9d9", pill_ink="#191919", menu="#f2f2f2", menu_bd="rgba(0,0,0,0.12)",
               shadow="0 1px 1px rgba(0,0,0,0.07), 0 8px 24px rgba(0,0,0,0.15)", card="#fcfcfc",
               dimmer="rgba(0,0,0,0.22)", caret="#00bfff"),
 "dark":  dict(name="quill", fs=13, hint_fs=11.5, ink="#cccccc", dim="#7e7e7e", strong="#bdbdbd", bg="#1a1a1a",
               rule="rgba(255,255,255,0.10)", row=32, sw=(32, 18, 14), sw_off="#3d3d3d", sw_on="#0a84c8", knob="#e8e8e8",
               knob_bd="rgba(0,0,0,0.3)", btn_bg="#262626", btn_bd="rgba(255,255,255,0.15)", btn_h=24, btn_pad=10, radius=5,
               field="#222222", accent="#0a84c8", check_bd="rgba(255,255,255,0.30)", list_bg="#151515", danger="#cf807e",
               track="#383838", head_fs=10.5, head_w=600, head_caps=True, sub_w=500,
               side="#141615", pill="#393b3a", pill_ink="#e0e0e0", menu="#2e2e2e", menu_bd="rgba(255,255,255,0.13)",
               shadow="0 1px 1px rgba(0,0,0,0.35), 0 10px 30px rgba(0,0,0,0.5)", card="#232323",
               dimmer="rgba(0,0,0,0.5)", caret="#00bfff"),
}

# ---------- the rows #463 leaves in Settings ----------
LOCATIONS = ["~/Documents/Manuscripts", "~/Documents/Notes", "~/Dropbox/Letters"]
PINNED = ["~/Documents/Manuscripts/The storm/Opening.md", "~/Documents/Notes/Harbour lights.md"]
TEMPLATES = ["Modern", "Classic", "Manuscript Mono", "Manuscript Duo", "Manuscript Quattro"]
NO_DICT = "No dictionary installed"
NO_DICT_HINT = ("No dictionary installed for en_GB: install hunspell-en_gb, or see the README's "
                "Spell check in other languages.")
REFUSED = ['is not TOML (line 28: duplicate key at "library.toggle"); keeping the settings Quill is running on',
           '"library.toggle" = ["<Super>l"]: <Super>l belongs to the compositor']

def R(t, l="", **kw):
    return dict(t=t, l=l, **kw)

def g_general():
    return [R("switch", "Follow System", on=True, hint="Light and dark follow the desktop"),
            R("switch", "Hide Bars", on=False, hint="The title and status bars fade while typing"),
            R("scale", "Typewriter anchor", val=0.50, lo=0.20, hi=0.80)]

def g_library():
    return [R("sub", "Locations"), R("paths", paths=LOCATIONS, add=True),
            R("sub", "Pinned"), R("paths", paths=PINNED, add=False),
            R("sub", "Files"),
            R("switch", "Show hidden folders", on=False), R("switch", "Show file extensions", on=False),
            R("switch", "Confirm before moving files", on=True), R("switch", "Always ask where to save", on=False)]

def g_template(as_radio):
    pick = ([R("sub", "Template")] + [R("radio", n, on=(n == "Modern")) for n in TEMPLATES] + [R("sub", "Layout")]
            if as_radio else [R("dropdown", "Template", val="Modern")])
    return pick + [R("switch", "Center headings", on=True), R("switch", "Number headings", on=False),
                   R("switch", "Indent paragraphs", on=False)]

def g_export():
    return [R("dropdown", "Paper", val="A4"), R("spin", "Margin (mm)", val=20), R("spin", "Text size (pt)", val=12),
            R("switch", "Title page", on=False), R("switch", "Header", on=True), R("switch", "Footer", on=True)]

def g_writing(no_dict=False):
    lang = (R("dropdown", "Language", val=NO_DICT, disabled=True, note=NO_DICT_HINT) if no_dict
            else R("dropdown", "Language", val="English (United Kingdom)"))
    return [R("sub", "Style check lists", hint="Supports English only for now."),
            R("check", "Fillers", on=True), R("check", "Redundancies", on=True), R("check", "Clichés", on=False),
            R("sub", "Spell check"), lang]

def g_advanced(refused=True):
    rows = [R("button", "Shortcuts and palette", val="Edit settings.toml…",
              hint="Shortcut rebinds and the palette file are set in the file")]
    if refused:
        rows += [R("sub", "Not applied from settings.toml"), R("refused", lines=REFUSED)]
    return rows

def groups(as_radio=True, no_dict=False, fold_advanced=False):
    gs = [("General", g_general()), ("Library", g_library()), ("Template", g_template(as_radio)),
          ("Export", g_export()), ("Writing tools", g_writing(no_dict)), ("Advanced", g_advanced())]
    if fold_advanced:  # tabs want fewer names: Advanced's button closes General
        gs[0] = ("General", g_general() + [R("sub", "Advanced")] + g_advanced())
        gs = gs[:5]
        gs[4] = ("Writing", gs[4][1])
    return gs

# ---------- glyphs ----------
def chev_down(col, s=10):
    return (f'<svg width="{s}" height="{s}" viewBox="0 0 10 10" fill="none" stroke="{col}" stroke-width="1.4" '
            f'stroke-linecap="round" stroke-linejoin="round" style="flex: none;"><path d="M2 3.5L5 6.5l3-3"></path></svg>')

def chev(col, d="right", s=10):
    p = "M3.5 2l3 3-3 3" if d == "right" else "M6.5 2l-3 3 3 3"
    return (f'<svg width="{s}" height="{s}" viewBox="0 0 10 10" fill="none" stroke="{col}" stroke-width="1.4" '
            f'stroke-linecap="round" stroke-linejoin="round" style="flex: none;"><path d="{p}"></path></svg>')

def pm(col, plus, s=12):
    p = "M2.5 6h7" + ("M6 2.5v7" if plus else "")
    return (f'<svg width="{s}" height="{s}" viewBox="0 0 12 12" fill="none" stroke="{col}" stroke-width="1.3" '
            f'stroke-linecap="round" style="flex: none;"><path d="{p}"></path></svg>')

def tick(col, s=10):
    return (f'<svg width="{s}" height="{s}" viewBox="0 0 10 10" fill="none" stroke="{col}" stroke-width="1.8" '
            f'stroke-linecap="round" stroke-linejoin="round" style="flex: none;"><path d="M2 5.2l2.2 2.2L8 3"></path></svg>')

def mag(col, s=14):
    return (f'<svg width="{s}" height="{s}" viewBox="0 0 13 13" fill="none" stroke="{col}" stroke-width="1.4" '
            f'stroke-linecap="round" style="flex: none;"><circle cx="5.5" cy="5.5" r="4"></circle><path d="M8.5 8.5l3 3"></path></svg>')

# ---------- widgets ----------
def switch(sk, on):
    w, h, k = sk["sw"]
    pad = (h - k) / 2
    left = w - k - pad if on else pad
    bd = f'box-shadow: inset 0 0 0 1px {sk["knob_bd"]};' if sk["name"] == "stock" else ""
    return (f'<div style="position: relative; width: {w}px; height: {h}px; border-radius: {h/2}px; flex: none; '
            f'background: {sk["sw_on"] if on else sk["sw_off"]}; {bd}">'
            f'<div style="position: absolute; top: {pad}px; left: {left}px; width: {k}px; height: {k}px; border-radius: 50%; '
            f'background: {sk["knob"]}; box-shadow: 0 0 0 1px {sk["knob_bd"]}, 0 1px 2px rgba(0,0,0,0.2);"></div></div>')

def button(sk, text, disabled=False, extra=""):
    op = "opacity: 0.5;" if disabled else ""
    return (f'<div style="display: flex; align-items: center; gap: 8px; height: {sk["btn_h"]}px; padding: 0 {sk["btn_pad"]}px; '
            f'box-sizing: border-box; border: 1px solid {sk["btn_bd"]}; border-radius: {sk["radius"]}px; '
            f'background: {sk["btn_bg"]}; color: {sk["ink"]}; font-size: {sk["fs"]}px; white-space: nowrap; flex: none; {op}">'
            f'{html.escape(text)}{extra}</div>')

def dropdown(sk, text, disabled=False):
    return button(sk, text, disabled, chev_down(sk["ink"]))

def spin(sk, val):
    h, bw = sk["btn_h"], (44 if sk["name"] == "stock" else 26)
    cell = (f'display: flex; align-items: center; justify-content: center; width: {bw}px; height: {h - 2}px; '
            f'border-left: 1px solid {sk["btn_bd"]};')
    return (f'<div style="display: flex; align-items: center; height: {h}px; box-sizing: border-box; border: 1px solid {sk["btn_bd"]}; '
            f'border-radius: {sk["radius"]}px; background: {sk["field"]}; color: {sk["ink"]}; font-size: {sk["fs"]}px; flex: none; '
            f'font-variant-numeric: tabular-nums;"><div style="width: {bw + 6}px; padding-left: 8px; box-sizing: border-box;">{val}</div>'
            f'<div style="{cell}">{pm(sk["ink"], False)}</div><div style="{cell}">{pm(sk["ink"], True)}</div></div>')

def scale(sk, val, lo, hi, width=170):
    stock = sk["name"] == "stock"
    th, k = (4, 20) if stock else (3, 14)
    x = (val - lo) / (hi - lo) * width
    return (f'<div style="display: flex; align-items: center; gap: 10px; flex: none;">'
            f'<div style="font-size: {sk["fs"] - 1}px; color: {sk["dim"]}; font-variant-numeric: tabular-nums;">{val:.2f}</div>'
            f'<div style="position: relative; width: {width}px; height: {k}px;">'
            f'<div style="position: absolute; left: 0; right: 0; top: {(k - th) / 2}px; height: {th}px; border-radius: {th}px; background: {sk["track"]};"></div>'
            f'<div style="position: absolute; left: 0; width: {x}px; top: {(k - th) / 2}px; height: {th}px; border-radius: {th}px; background: {sk["accent"]};"></div>'
            f'<div style="position: absolute; left: {x - k / 2}px; top: 0; width: {k}px; height: {k}px; border-radius: 50%; box-sizing: border-box; '
            f'background: {sk["knob"]}; box-shadow: 0 0 0 1px {sk["knob_bd"]}, 0 1px 2px rgba(0,0,0,0.25);"></div></div></div>')

def mark(sk, on, round_):
    s = 16 if sk["name"] == "stock" else 14
    rad = "50%" if round_ else "3px"
    if on:
        inner = (f'<div style="width: {s * 0.38}px; height: {s * 0.38}px; border-radius: 50%; background: #ffffff;"></div>'
                 if round_ else tick("#ffffff", s - 4))
        return (f'<div style="display: flex; align-items: center; justify-content: center; width: {s}px; height: {s}px; '
                f'border-radius: {rad}; background: {sk["accent"]}; flex: none;">{inner}</div>')
    return (f'<div style="width: {s}px; height: {s}px; box-sizing: border-box; border-radius: {rad}; '
            f'border: 1px solid {sk["check_bd"]}; background: {sk["field"]}; flex: none;"></div>')

# ---------- rows: each returns (html, height) ----------
def label(sk, text, hint=None):
    h = (f'<div style="font-size: {sk["hint_fs"]}px; color: {sk["dim"]}; margin-top: 1px; white-space: nowrap; overflow: hidden; '
         f'text-overflow: ellipsis;">{html.escape(hint)}</div>') if hint else ""
    return (f'<div style="flex: 1; min-width: 0;"><div style="font-size: {sk["fs"]}px; color: {sk["ink"]}; white-space: nowrap;">'
            f'{html.escape(text)}</div>{h}</div>')

def line(sk, left, right, h, pad=0):
    return (f'<div style="display: flex; align-items: center; gap: 16px; height: {h}px; padding: 0 {pad}px; box-sizing: border-box;">'
            f'{left}{right}</div>', h)

def render_row(r, sk, pad=0, hints=True):
    t, rh = r["t"], sk["row"]
    hint = r.get("hint") if hints else None
    tall = rh + (14 if hint else 0)
    if t == "switch":
        return line(sk, label(sk, r["l"], hint), switch(sk, r["on"]), tall, pad)
    if t == "scale":
        return line(sk, label(sk, r["l"]), scale(sk, r["val"], r["lo"], r["hi"], r.get("w", 170)), rh, pad)
    if t == "spin":
        return line(sk, label(sk, r["l"]), spin(sk, r["val"]), rh + (4 if sk["name"] == "stock" else 0), pad)
    if t == "button":
        return line(sk, label(sk, r["l"], hint), button(sk, r["val"]), tall + (4 if sk["name"] == "stock" else 0), pad)
    if t == "dropdown":
        h0 = rh + (4 if sk["name"] == "stock" else 0)
        body, h = line(sk, label(sk, r["l"]), dropdown(sk, r["val"], r.get("disabled", False)), h0, pad)
        if r.get("note"):
            nh = 34
            body += (f'<div style="height: {nh}px; padding: 0 {pad}px; box-sizing: border-box; font-size: {sk["hint_fs"]}px; '
                     f'line-height: 1.35; color: {sk["dim"]};">{html.escape(r["note"])}</div>')
            h += nh
        return body, h
    if t in ("check", "radio"):
        h = rh - 4
        return (f'<div style="display: flex; align-items: center; gap: 9px; height: {h}px; padding: 0 {pad}px; box-sizing: border-box; '
                f'font-size: {sk["fs"]}px; color: {sk["ink"]};">{mark(sk, r["on"], t == "radio")}{html.escape(r["l"])}</div>', h)
    if t == "paths":
        out, h = "", 0
        for i, p in enumerate(r["paths"]):
            sep = f'border-top: 1px solid {sk["rule"]};' if i else ""
            out += (f'<div style="display: flex; align-items: center; gap: 12px; height: {rh}px; padding: 0 {4 if sk["name"] == "stock" else 3}px 0 10px; '
                    f'box-sizing: border-box; {sep}"><div style="flex: 1; min-width: 0; font-size: {sk["fs"]}px; color: {sk["ink"]}; white-space: nowrap; '
                    f'overflow: hidden; text-overflow: ellipsis; direction: rtl; text-align: left;">&lrm;{html.escape(p)}</div>'
                    f'{button(dict(sk, btn_h=sk["btn_h"] - (6 if sk["name"] == "stock" else 2)), "Remove")}</div>')
            h += rh
        box = (f'<div style="margin: 0 {pad}px; border: 1px solid {sk["btn_bd"]}; border-radius: {sk["radius"] + 1}px; '
               f'background: {sk["list_bg"]}; overflow: hidden;">{out}</div>')
        h += 2
        if r.get("add"):
            ah = sk["btn_h"] + 8
            box += (f'<div style="display: flex; justify-content: flex-end; align-items: flex-end; height: {ah}px; padding: 0 {pad}px; '
                    f'box-sizing: border-box;">{button(sk, "Add…")}</div>')
            h += ah
        return box, h
    if t == "refused":
        n = len(r["lines"])
        h = 12 + n * 34
        body = "".join(f'<div style="height: 34px; overflow: hidden;">{html.escape(l)}</div>' for l in r["lines"])
        return (f'<div style="height: {h}px; margin: 0 {pad}px; padding: 6px 10px; box-sizing: border-box; border-radius: {sk["radius"]}px; '
                f'border: 1px solid {sk["btn_bd"]}; font-family: {MONO}; font-size: 11px; line-height: 17px; color: {sk["danger"]}; '
                f'overflow: hidden;">{body}</div>', h)
    raise ValueError(t)

def head(sk, text, hint=None, big=False, pad=0, first=False):
    """A group heading (big) or a sub-label inside a pane."""
    top = 0 if first else (22 if big else 14)
    h = top + 26
    caps = "text-transform: uppercase; letter-spacing: 0.06em;" if sk["head_caps"] else ""
    if big:
        style = (f'font-size: 17px; font-weight: 700; color: {sk["ink"]};' if sk["name"] == "stock"
                 else f'font-size: 15px; font-weight: 600; color: {sk["ink"]};')
    else:
        style = (f'font-size: {sk["fs"]}px; font-weight: 700; color: {sk["ink"]};' if sk["name"] == "stock"
                 else f'font-size: {sk["head_fs"]}px; font-weight: {sk["head_w"]}; color: {sk["dim"]}; {caps}')
    hint_html = (f'<span style="font-size: {sk["hint_fs"]}px; font-weight: 400; color: {sk["dim"]}; text-transform: none; '
                 f'letter-spacing: 0; margin-left: 10px;">{html.escape(hint)}</span>') if hint else ""
    return (f'<div style="height: {h}px; padding: {top}px {pad}px 0; box-sizing: border-box; line-height: 22px; white-space: nowrap; {style}">'
            f'{html.escape(text)}{hint_html}</div>', h)

def render_rows(rows, sk, boxed=False, seps=False, hints=True):
    """Rows top to bottom. boxed wraps each run between sub-labels in a card (a ListBox with CSS)."""
    out, total, run = "", 0, []
    def flush():
        nonlocal out, total, run
        if not run:
            return
        body, h = "", 0
        for i, (b, bh) in enumerate(run):
            if (boxed or seps) and i:
                body += f'<div style="height: 1px; margin: 0 {10 if boxed else 0}px; background: {sk["rule"]};"></div>'
                h += 1
            body += b
            h += bh
        if boxed:
            body = (f'<div style="border: 1px solid {sk["btn_bd"]}; border-radius: 8px; background: {sk["card"]}; overflow: hidden;">{body}</div>')
            h += 2
        out += body
        total += h
        run = []
    first = True
    for r in rows:
        if r["t"] == "sub":
            flush()
            b, h = head(sk, r["l"], r.get("hint"), first=first)
            out += b
            total += h
        elif r["t"] == "paths" and boxed:
            flush()
            b, h = render_row(r, sk)
            out += b
            total += h
        else:
            run.append(render_row(r, sk, pad=10 if boxed else 0, hints=hints))
        first = False
    flush()
    return out, total

# ---------- the Quill main window, for context ----------
MW, MH = 1120, 720
PROSE = [["She left it under the lamp, where he would have to move it to",
          "read anything else. Four lines, and three of them crossed out."],
         ["What was left said only that the boat had come back empty, and",
          "that she had gone down to the water to wait. He read it twice,",
          "and then he read the crossed-out lines, which is what she had",
          "meant him to do."]]

def main_window(sk, overlay=""):
    lines = (f'<div style="position: relative; font-weight: 700; height: 37px;"><span style="position: absolute; left: -26px; ">#</span>'
             f'<span style="position: absolute; left: -32px; top: 3px; width: 2px; height: 30px; background: {sk["caret"]}; opacity: 0.6;"></span>'
             f'The Note on the Table</div><div style="height: 37px;"></div>')
    for para in PROSE:
        lines += "".join(f'<div style="height: 37px; white-space: nowrap;">{html.escape(l)}</div>' for l in para) + '<div style="height: 37px;"></div>'
    bar_icon = (f'<svg width="14" height="12" viewBox="0 0 14 12" fill="none" stroke="{sk["dim"]}" stroke-width="1.1"><rect x="0.6" y="0.6" width="12.8" height="10.8" rx="2"></rect>'
                f'<path d="M5 0.6v10.8"></path></svg>')
    menu_icon = (f'<svg width="22" height="12" viewBox="0 0 22 12" fill="none" stroke="{sk["dim"]}" stroke-width="1.1" stroke-linecap="round">'
                 f'<path d="M1 2h13M1 5h13M1 8h13M1 11h8M17 5l2 2 2-2"></path></svg>')
    return (f'<div style="position: relative; width: {MW}px; height: {MH}px; overflow: hidden; background: {sk["bg"]}; font-family: {SANS};">'
            f'<div style="position: absolute; left: 16px; top: 14px;">{bar_icon}</div>'
            f'<div style="position: absolute; right: 14px; top: 14px;">{menu_icon}</div>'
            f'<div style="position: absolute; left: 0; right: 0; top: 10px; text-align: center; font-size: 14px; color: {sk["dim"]};">short</div>'
            f'<div style="position: absolute; left: 236px; top: 76px; font-family: {MONO}; font-size: 17px; line-height: 37px; color: {sk["ink"]};">{lines}</div>'
            f'<div style="position: absolute; left: 0; right: 0; bottom: 0; height: 26px; border-top: 1px solid {sk["rule"]}; display: flex; '
            f'justify-content: center; align-items: center; gap: 16px; font-size: 11.5px; color: {sk["ink"]};">'
            f'<span>73 words</span><span>349 characters</span><span>&lt; 1 min read</span></div>{overlay}</div>')

# ---------- directions ----------
def a1_window(sk, gs, current, w=700, h=520):
    side = ""
    for name, _ in gs:
        on = name == current
        side += (f'<div style="display: flex; align-items: center; height: 28px; margin: 0 8px 1px; padding: 0 9px; border-radius: 5px; '
                 f'font-size: {sk["fs"]}px; color: {sk["pill_ink"] if on else sk["ink"]}; font-weight: {600 if on else 400}; '
                 f'background: {sk["pill"] if on else "transparent"};">{html.escape(name)}</div>')
    body, _ = render_rows(dict(gs)[current], sk, seps=False)
    return (f'<div style="display: flex; width: {w}px; height: {h}px; overflow: hidden; background: {sk["bg"]}; font-family: {SANS}; box-sizing: border-box;">'
            f'<div style="width: 168px; flex: none; background: {sk["side"]}; border-right: 1px solid {sk["rule"]}; padding-top: 14px; box-sizing: border-box;">'
            f'<div style="padding: 0 17px 8px; font-size: {sk["head_fs"]}px; font-weight: 600; letter-spacing: 0.06em; text-transform: uppercase; color: {sk["dim"]};">Settings</div>{side}</div>'
            f'<div style="flex: 1; min-width: 0; padding: 18px 28px; box-sizing: border-box;">{body}</div></div>')

def a2_window(sk, gs, current, w=640, h=560):
    tabs = ""
    for i, (name, _) in enumerate(gs):
        on = name == current
        bd = f'border-left: 1px solid {sk["btn_bd"]};' if i else ""
        tabs += (f'<div style="display: flex; align-items: center; height: 24px; padding: 0 14px; font-size: {sk["fs"]}px; {bd} '
                 f'color: {sk["pill_ink"] if on else sk["ink"]}; font-weight: {600 if on else 400}; background: {sk["pill"] if on else sk["btn_bg"]};">{html.escape(name)}</div>')
    body, _ = render_rows(dict(gs)[current], sk, seps=True)
    return (f'<div style="width: {w}px; height: {h}px; overflow: hidden; background: {sk["bg"]}; font-family: {SANS};">'
            f'<div style="display: flex; justify-content: center; align-items: center; height: 52px; border-bottom: 1px solid {sk["rule"]};">'
            f'<div style="display: flex; border: 1px solid {sk["btn_bd"]}; border-radius: 6px; overflow: hidden;">{tabs}</div></div>'
            f'<div style="padding: 14px 72px;">{body}</div></div>')

def floating(sk, inner, w, h):
    return (f'<div style="position: absolute; left: {(MW - w) / 2}px; top: {(MH - h) / 2}px; width: {w}px; height: {h}px; '
            f'border: 1px solid {sk["menu_bd"]}; box-shadow: 0 18px 50px rgba(0,0,0,0.35); box-sizing: content-box;">{inner}</div>')

def b1_sheet(sk, gs, current):
    w, h = 560, 600
    tabs = ""
    for name, _ in gs:
        on = name == current
        tabs += (f'<div style="height: 36px; line-height: 36px; font-size: {sk["fs"]}px; color: {sk["ink"] if on else sk["dim"]}; '
                 f'font-weight: {600 if on else 400}; box-shadow: {"inset 0 -2px 0 " + sk["accent"] if on else "none"}; white-space: nowrap;">{html.escape(name)}</div>')
    body, _ = render_rows(dict(gs)[current], dict(sk, list_bg=sk["card"]), boxed=True)
    sheet = (f'<div style="position: absolute; left: {(MW - w) / 2}px; top: 56px; width: {w}px; height: {h}px; box-sizing: border-box; '
             f'display: flex; flex-direction: column; background: {sk["menu"]}; border: 1px solid {sk["menu_bd"]}; border-radius: 10px; box-shadow: {sk["shadow"]}; overflow: hidden;">'
             f'<div style="display: flex; justify-content: center; gap: 22px; padding: 4px 20px 0; border-bottom: 1px solid {sk["rule"]}; flex: none;">{tabs}</div>'
             f'<div style="flex: 1; padding: 8px 24px 0; overflow: hidden;">{body}</div>'
             f'<div style="display: flex; justify-content: flex-end; padding: 10px 24px 14px; flex: none;">{button(sk, "Done")}</div></div>')
    return main_window(sk, f'<div style="position: absolute; inset: 0; background: {sk["dimmer"]};"></div>{sheet}')

def b2_pane(sk, gs, current):
    w = 360
    if current is None:
        inner = ""
        for name, rows in gs:
            n = sum(1 for r in rows if r["t"] not in ("sub",))
            inner += (f'<div style="display: flex; align-items: center; height: 40px; border-bottom: 1px solid {sk["rule"]}; font-size: {sk["fs"]}px; color: {sk["ink"]};">'
                      f'<div style="flex: 1;">{html.escape(name)}</div>{chev(sk["dim"])}</div>')
        title = f'<div style="font-size: {sk["fs"] + 1}px; font-weight: 600; color: {sk["ink"]};">Settings</div>'
    else:
        inner, _ = render_rows(dict(gs)[current], sk, hints=False)
        title = (f'<div style="display: flex; align-items: center; gap: 6px; font-size: {sk["fs"]}px; color: {sk["dim"]};">{chev(sk["dim"], "left")}Settings</div>'
                 f'<div style="flex: 1; text-align: center; font-size: {sk["fs"] + 1}px; font-weight: 600; color: {sk["ink"]}; margin-right: 58px;">{html.escape(current)}</div>')
    pane = (f'<div style="position: absolute; right: 0; top: 0; bottom: 0; width: {w}px; box-sizing: border-box; background: {sk["menu"]}; '
            f'border-left: 1px solid {sk["menu_bd"]}; box-shadow: -8px 0 24px rgba(0,0,0,{0.10 if sk is QUILL["light"] else 0.4});">'
            f'<div style="display: flex; align-items: center; height: 44px; padding: 0 18px; border-bottom: 1px solid {sk["rule"]};">{title}</div>'
            f'<div style="padding: 8px 18px;">{inner}</div></div>')
    return main_window(sk, pane)

def d_palette(sk, gs, query):
    w = 600
    flat = []
    for name, rows in gs:
        sub = None
        for r in rows:
            if r["t"] == "sub":
                sub = r["l"]
                continue
            if r["t"] == "paths":
                flat.append((name, R("drill", sub, val=f'{len(r["paths"])} {"folders" if sub == "Locations" else "files"}')))
            elif r["t"] == "radio":
                if r["on"]:
                    flat.append((name, R("dropdown", "Template", val=r["l"])))
            elif r["t"] == "refused":
                flat.append((name, R("drill", "Not applied from settings.toml", val=f'{len(r["lines"])} lines', warn=True)))
            else:
                flat.append((name, r))
    if query:
        flat = [(g, r) for g, r in flat if query.lower() in (g + " " + r["l"]).lower()]
    rowsh, shown = "", 0
    for i, (g, r) in enumerate(flat[:13]):
        sel = i == 0
        ink = "#ffffff" if sel else sk["ink"]
        dim = "rgba(255,255,255,0.8)" if sel else sk["dim"]
        rs = dict(sk, ink=ink) if sel and r["t"] in ("check",) else sk
        if r["t"] == "switch":
            ctl = switch(dict(sk, sw_off="rgba(255,255,255,0.35)", sw_on="rgba(255,255,255,0.35)") if sel else sk, r["on"])
        elif r["t"] == "check":
            ctl = mark(sk, r["on"], False)
        elif r["t"] == "dropdown":
            ctl = dropdown(sk, r["val"], r.get("disabled", False))
        elif r["t"] == "spin":
            ctl = spin(sk, r["val"])
        elif r["t"] == "scale":
            ctl = scale(sk, r["val"], r["lo"], r["hi"], 120)
        elif r["t"] == "button":
            ctl = button(sk, r["val"])
        else:
            ctl = (f'<div style="display: flex; align-items: center; gap: 6px; font-size: {sk["fs"] - 1}px; color: {sk["danger"] if r.get("warn") and not sel else dim};">'
                   f'{html.escape(r["val"])}{chev(dim)}</div>')
        rowsh += (f'<div style="display: flex; align-items: center; gap: 10px; height: 34px; margin: 0 6px; padding: 0 10px; border-radius: 5px; '
                  f'background: {sk["accent"] if sel else "transparent"};"><div style="width: 92px; flex: none; font-size: {sk["fs"] - 1.5}px; color: {dim};">{html.escape(g)}</div>'
                  f'<div style="flex: 1; font-size: {sk["fs"]}px; color: {ink}; white-space: nowrap;">{html.escape(r["l"])}</div>{ctl}</div>')
        shown += 1
    more = (f'<div style="height: 24px; line-height: 24px; text-align: center; font-size: {sk["hint_fs"]}px; color: {sk["dim"]};">{len(flat) - shown} more below</div>'
            if len(flat) > shown else "")
    caret = f'<span style="display: inline-block; width: 1.5px; height: 17px; margin-left: 1px; vertical-align: -3px; background: {sk["caret"]};"></span>'
    field = (f'<div style="display: flex; align-items: center; gap: 10px; height: 46px; padding: 0 16px; border-bottom: 1px solid {sk["rule"]};">{mag(sk["dim"])}'
             f'<div style="font-size: 15px; color: {sk["ink"] if query else sk["dim"]};">{html.escape(query) if query else "Search settings"}'
             f'{caret if query else ""}</div></div>')
    box = (f'<div style="position: absolute; left: {(MW - w) / 2}px; top: 72px; width: {w}px; box-sizing: border-box; background: {sk["menu"]}; '
           f'border: 1px solid {sk["menu_bd"]}; border-radius: 10px; box-shadow: {sk["shadow"]}; overflow: hidden;">{field}'
           f'<div style="padding: 6px 0;">{rowsh}{more}</div></div>')
    return main_window(sk, f'<div style="position: absolute; inset: 0; background: {sk["dimmer"]}; opacity: 0.6;"></div>{box}')

CW = 560
FOLD = 640
def c_window(sk, gs):
    pad = 28
    out, total = "", 20
    for i, (name, rows) in enumerate(gs):
        b, h = head(sk, name, big=True, first=(i == 0))
        rb, rh = render_rows(rows, sk)
        rule = ""
        if i:
            rule = f'<div style="height: 1px; margin-top: 18px; background: {sk["rule"]};"></div>'
            total += 19
        out += rule + b + rb
        total += h + rh
    total += 24
    fold = (f'<div style="position: absolute; left: 0; right: 0; top: {FOLD}px; border-top: 1px dashed #e0457b;"></div>'
            f'<div style="position: absolute; left: 270px; top: {FOLD - 7}px; padding: 0 6px; background: {sk["bg"]}; font-size: 10px; line-height: 14px; color: #e0457b;">a 640 px window ends here; the rest scrolls</div>')
    return (f'<div style="position: relative; width: {CW}px; height: {total}px; overflow: hidden; background: {sk["bg"]}; font-family: {SANS}; '
            f'padding: 20px {pad}px 0; box-sizing: border-box;">{out}{fold}</div>', total)

# ---------- artboards ----------
def page(body, bg):
    return f'''<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
  {FONTS}
  <style>
    body {{ margin: 0; background: {bg}; font-family: {SANS}; -webkit-font-smoothing: antialiased; }}
  </style>
</helmet>
{body}
</x-dc>
</body>
</html>
'''

NOTES = {
 "A1": ("A1. A separate window, sidebar of panes",
        "gtk::Window + Stack + StackSidebar (or a ListBox driving the Stack). Six panes: General, Library, Template, Export, Writing tools, Advanced. "
        "Rows are short; nothing scrolls at 700 x 520. Template's five are radio CheckButtons. The sidebar takes the Library Organizer's ground and pill. "
        "Tradeoff: a second window to place and lose; on a tiling compositor it needs a float rule."),
 "A2": ("A2. A separate window, switcher on top",
        "gtk::Window + Stack + StackSwitcher, drawn as a plain linked row in the content (no GtkHeaderBar). Tabs want fewer names, so Advanced folds into "
        "the foot of General and Writing tools shortens to Writing: five tabs. Hairlines between rows. Tradeoff: a sixth pane has nowhere to go; the window is as wide as its tabs."),
 "B1": ("B1. A sheet over a dimmed Editor",
        "gtk::Overlay on the window: a dimming box plus a centred box with the menu's ground, border, radius and shadow; text tabs over a Stack; boxed rows are a ListBox with CSS; Esc or Done closes. "
        "No second window, so nothing to float or lose, and it is the same on every platform. Tradeoff: it hides the page, so Typewriter anchor and the Template cannot be judged while changing them."),
 "B2": ("B2. A side pane over the Editor",
        "Overlay + Revealer sliding from the right, 360 px, the menu's ground. Drill-in: a root list of panes, then one pane with a back row (a Stack with a slide transition). "
        "The page stays visible and live, so Template and anchor changes show as they are made. Tradeoff: one more level of navigation, long paths ellipsize at 360 px, and it competes with Preview Split for the right edge."),
 "C1": ("C1. One scrolling window, regrouped, stock GTK4",
        "Today's window with #463's rows under six headings, in the default theme exactly as sampled from today's shots: 14.67 px type, 38 px pitch, 50 x 26 switches, #3584e4 (light) and #15539e (dark). The honest baseline: regrouping only, no CSS. Shown unrolled; the dashed line is where a 640 px window ends."),
 "C2": ("C2. The identical layout, Quill's CSS",
        "Same rows, same order, same widgets, same 560 px width as C1. Only a stylesheet differs: 13 px chrome type, 32 px pitch, 32 x 18 switches in the menu's selection blue, flat hairline buttons, the View menu's small-caps heads, the Library's list ground. "
        "Everything here is reachable with a CssProvider on Switch, Button, DropDown, SpinButton, Scale, CheckButton and a framed ListBox."),
 "D":  ("D. A searchable settings list",
        "The Palette's shape: an Entry over one flat ListBox of every row, the group as a dim prefix, the control inline, filtered as you type. No panes to name or learn. "
        "Tradeoff: path lists and the refused lines cannot sit inline, so they become drill-in rows; browsing 27 rows is worse than six panes; keyboard focus moves between the Entry and inline controls need care."),
}

def main():
    boards, notes = [], []
    x_gap, y = 80, 0
    def add(stem, title, body, w, h, col_x, bg):
        fn = f"{stem}.dc.html"
        with open(os.path.join(OUT, fn), "w") as f:
            f.write(page(body, bg))
        boards.append({"file": fn, "x": col_x, "y": y, "w": w, "h": h, "title": title})
        return col_x + w + x_gap
    def note(key):
        t, body = NOTES[key]
        notes.append({"id": f"dir-{key}", "x": -320, "y": y + 20, "w": 280, "text": f"{t}\n\n{body}"})

    gs6 = groups(as_radio=True)
    gs6_nodict = groups(as_radio=True, no_dict=True)
    gs5 = groups(as_radio=True, fold_advanced=True)
    gsC = groups(as_radio=False)

    # A1
    note("A1"); x = 0
    for pane in ("Library", "Export"):
        for g in ("light", "dark"):
            sk = QUILL[g]
            stem = "Main" if (pane == "Library" and g == "light") else f"A1{pane}{g.capitalize()}"
            x = add(stem, f"A1 sidebar · {pane} · {g}", a1_window(sk, gs6, pane), 700, 520, x, sk["bg"])
    y += 520 + 140; x = 0
    for pane, gs, tag in (("General", gs6, "General"), ("Template", gs6, "Template"), ("Writing tools", gs6, "Writing"),
                          ("Writing tools", gs6_nodict, "WritingNoDictionary"), ("Advanced", gs6, "Advanced")):
        sk = QUILL["light"]
        x = add(f"A1{tag}Light", f"A1 sidebar · {tag} · light", a1_window(sk, gs, pane), 700, 520, x, sk["bg"])
    y += 520 + 140; x = 0
    for g in ("light", "dark"):
        sk = QUILL[g]
        x = add(f"A1Context{g.capitalize()}", f"A1 sidebar · floating over the main window · {g}",
                main_window(sk, floating(sk, a1_window(sk, gs6, "Library"), 700, 520)), MW, MH, x, sk["bg"])
    y += MH + 140

    # A2
    note("A2"); x = 0
    for pane in ("Library", "Export", "General"):
        for g in ("light", "dark"):
            if pane == "General" and g == "dark":
                continue
            sk = QUILL[g]
            x = add(f"A2{pane}{g.capitalize()}", f"A2 switcher · {pane} · {g}", a2_window(sk, gs5, pane), 640, 560, x, sk["bg"])
    y += 560 + 140

    # B1, B2
    for key, fn, panes in (("B1", b1_sheet, ("Library", "Export")), ("B2", b2_pane, (None, "Library", "Export"))):
        note(key); x = 0
        for pane in panes:
            for g in ("light", "dark"):
                if pane is None and g == "dark":
                    continue
                sk = QUILL[g]
                x = add(f"{key}{pane or 'Root'}{g.capitalize()}", f"{key} · {pane or 'root list'} · {g}", fn(sk, gs6 if key == "B2" else gs5, pane), MW, MH, x, sk["bg"])
        y += MH + 140

    # C1 against C2, side by side per ground
    note("C1"); notes.append({"id": "dir-C2", "x": -320, "y": y + 420, "w": 280, "text": NOTES["C2"][0] + "\n\n" + NOTES["C2"][1]})
    x, tallest = 0, 0
    for g in ("light", "dark"):
        for key, skin in (("C1", STOCK), ("C2", QUILL)):
            sk = skin[g]
            body, h = c_window(sk, gsC)
            x = add(f"{key}{g.capitalize()}", f"{key} {'stock GTK4' if key == 'C1' else 'Quill CSS'} · {g}", body, CW, h, x, sk["bg"])
            tallest = max(tallest, h)
    y += tallest + 140

    # D
    note("D"); x = 0
    for query, tag in (("", "Rest"), ("head", "Query")):
        for g in ("light", "dark"):
            if tag == "Rest" and g == "dark":
                continue
            sk = QUILL[g]
            x = add(f"D{tag}{g.capitalize()}", f"D searchable list · {tag.lower()} · {g}", d_palette(sk, gsC, query), MW, MH, x, sk["bg"])

    canvas = {"artboards": boards, "annotations": notes, "launch": {"view": "canvas"}}
    with open(os.path.join(OUT, "canvas.json"), "w") as f:
        json.dump(canvas, f, indent=1)
    print(len(boards), "artboards")

if __name__ == "__main__":
    main()
