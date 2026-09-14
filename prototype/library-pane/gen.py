#!/usr/bin/env python3
"""Generate the Library pane direction artboards (.dc.html) and canvas.json.

Six directions x three states x two grounds, plus excerpts-off and no-bar on
light for the two leading directions. Every value in the iA-faithful direction
is the oracle's (ref/ia/mac-native/NOTES.md § State 28 and the #436 frame
readings), at 1 CSS px per pt.
"""
import json, os, html

OUT = os.path.dirname(os.path.abspath(__file__))
W, H = 760, 640          # artboard: 360 pane + 400 of page
PANE = 360
ORG = 130                # Organizer width (two-column directions)
LIST = PANE - ORG
TITLE_H = 52
SORT_H = 33
ROW = 68
ROW_BARE = 32
FOLDER = 32

SANS = "'Adwaita Sans', Inter, system-ui, sans-serif"
SERIF = "'Source Serif 4', Georgia, serif"
MONO = "'iA Writer Mono', 'JetBrains Mono', ui-monospace, monospace"

# ---------- content ----------
DOCS = [
    ("The storm", "Mar 14", "The storm The gale came up the coast at four in the morning and took the roof off the boathouse. By noon", True),
    ("Harbour lights", "Mar 12", "Harbour lights The sea was flat all evening and the lamps along the quay doubled themselves in it. Nobody was out.", False),
    ("Quiet morning", "Mar 10", "Quiet morning Coffee, the radio low, and an hour before anyone else is awake. Three paragraphs today, and none", False),
    ("Letters, unsent", "Mar 8", "Letters, unsent Dear R — I have started this four times now and thrown away three. The fourth is this one.", False),
    ("Field notes", "Mar 6", "Field notes Rain on the third day. The path above the quarry is washed out; go by the lane instead. Heard a curlew", False),
    ("Winter list", "Mar 4", "Winter list Firewood. Storm windows. The gutter over the porch. And, if the week allows it, the long chapter", False),
]
SEARCH_DOCS = [DOCS[0], DOCS[1], DOCS[4]]
LOCATIONS = ["Manuscripts", "Notes", "Letters"]
PINNED = ["Opening", "Harbour lights"]

# ---------- palettes ----------
# keys: org, list, page, title_list (title bar over list), ink, sec, sep, head, pill, pill_ink,
#       rule, bar, sortbg, sortbd, sortink, fieldbg, fieldbd, icon, rail_ink
P = {
 "A": {  # iA, faithfully — every value measured
  "light": dict(org="#eaebeb", list="#fcfcfc", page="#f7f7f7", title_list="#fcfcfc", title_page="#fcfcfc",
                ink="#191919", sec="#999999", sep="#ededed", head="#7f8080", pill="#d8d9d9", pill_ink="#191919",
                rule="#e0e0e0", bar="#36bffa", sortbg="#f6f6f6", sortbd="#e8e8e8", sortink="#767676",
                fieldbg="#fcfcfc", fieldbd="#dbdbdb", icon="#7e7e7e", title_ink="#545454", folder="#72b9e5",
                docfill="#f5f5f5", docedge="#d7d7d7"),
  "dark":  dict(org="#1a1c1b", list="#151515", page="#1a1a1a", title_list="#1c1c1c", title_page="#222222",
                ink="#b9b9b9", sec="#757575", sep="#212121", head="#6a6c6b", pill="#393b3a", pill_ink="#e0e0e0",
                rule="#252525", bar="#36bffa", sortbg="#3a3a3a", sortbd="#686868", sortink="#9c9c9c",
                fieldbg="#161616", fieldbd="#2e2e2e", icon="#939393", title_ink="#c6c4c7", folder="#70b7e3",
                docfill="#f5f5f5", docedge="#d7d7d7"),
 },
 "B": {  # iA's structure in Quill's greys — theme.rs values, two grounds
  "light": dict(org="#ededec", list="#f7f7f7", page="#f7f7f7", title_list="#f7f7f7", title_page="#f7f7f7",
                ink="#191919", sec="#8c8c8c", sep="#e6e5e3", head="#8c8c8c", pill=None, pill_ink="#191919",
                rule="#e2e1df", bar="#00bfff", sortbg=None, sortbd=None, sortink="#8c8c8c",
                fieldbg="#f7f7f7", fieldbd="#dcdbd9", icon="#8c8c8c", title_ink="#4a4a4a", folder="#00bfff",
                docfill="#f7f7f7", docedge="#c6c4c2"),
  "dark":  dict(org="#141414", list="#1a1a1a", page="#1a1a1a", title_list="#1a1a1a", title_page="#1a1a1a",
                ink="#cccccc", sec="#7e7e7e", sep="#262626", head="#7e7e7e", pill=None, pill_ink="#cccccc",
                rule="#2a2a2a", bar="#00bfff", sortbg=None, sortbd=None, sortink="#7e7e7e",
                fieldbg="#1a1a1a", fieldbd="#333333", icon="#7e7e7e", title_ink="#bdbdbd", folder="#00bfff",
                docfill="#1a1a1a", docedge="#707070"),
 },
 "C": {  # one column with a Location head — the oracle's list values, no Organizer
  "light": None, "dark": None,  # filled from A below
 },
 "D": {  # a drawer — the Organizer as dark furniture
  "light": dict(org="#2b2b2b", list="#fcfcfc", page="#f7f7f7", title_list="#fcfcfc", title_page="#fcfcfc",
                ink="#191919", sec="#999999", sep="#ededed", head="#8a8a8a", pill="#3f3f3f", pill_ink="#ffffff",
                rule="#e0e0e0", bar="#36bffa", sortbg="#f6f6f6", sortbd="#e8e8e8", sortink="#767676",
                fieldbg="#fcfcfc", fieldbd="#dbdbdb", icon="#7e7e7e", title_ink="#545454", folder="#72b9e5",
                docfill="#f5f5f5", docedge="#d7d7d7", org_ink="#d6d6d6"),
  "dark":  dict(org="#262626", list="#151515", page="#1a1a1a", title_list="#1c1c1c", title_page="#222222",
                ink="#b9b9b9", sec="#757575", sep="#212121", head="#7a7a7a", pill="#3a3a3a", pill_ink="#f0f0f0",
                rule="#252525", bar="#36bffa", sortbg="#3a3a3a", sortbd="#686868", sortink="#9c9c9c",
                fieldbg="#161616", fieldbd="#2e2e2e", icon="#939393", title_ink="#c6c4c7", folder="#70b7e3",
                docfill="#f5f5f5", docedge="#d7d7d7", org_ink="#d0d0d0"),
 },
 "E": {  # editorial — a manuscript index, serif names, no icons
  "light": dict(org="#f1efea", list="#faf8f4", page="#f7f7f7", title_list="#faf8f4", title_page="#f7f7f7",
                ink="#1c1a17", sec="#8a857c", sep="#e6e2da", head="#8a857c", pill=None, pill_ink="#1c1a17",
                rule="#e6e2da", bar="#1c1a17", sortbg=None, sortbd=None, sortink="#8a857c",
                fieldbg="#faf8f4", fieldbd="#dcd7cc", icon="#8a857c", title_ink="#1c1a17", folder="#8a857c",
                docfill="#faf8f4", docedge="#c9c3b6"),
  "dark":  dict(org="#1f1e1c", list="#171614", page="#1a1a1a", title_list="#171614", title_page="#1a1a1a",
                ink="#d3cec5", sec="#7d7873", sep="#2a2825", head="#7d7873", pill=None, pill_ink="#d3cec5",
                rule="#2a2825", bar="#d3cec5", sortbg=None, sortbd=None, sortink="#7d7873",
                fieldbg="#171614", fieldbd="#33302c", icon="#7d7873", title_ink="#d3cec5", folder="#7d7873",
                docfill="#171614", docedge="#5a5650"),
 },
 "F": {  # a rail — Locations, Pinned and Recents as a 56 px icon rail
  "light": dict(org="#e6e7e7", list="#fcfcfc", page="#f7f7f7", title_list="#fcfcfc", title_page="#fcfcfc",
                ink="#191919", sec="#999999", sep="#ededed", head="#7f8080", pill="#fcfcfc", pill_ink="#191919",
                rule="#e0e0e0", bar="#36bffa", sortbg="#f6f6f6", sortbd="#e8e8e8", sortink="#767676",
                fieldbg="#fcfcfc", fieldbd="#dbdbdb", icon="#7e7e7e", title_ink="#545454", folder="#72b9e5",
                docfill="#f5f5f5", docedge="#d7d7d7", rail_ink="#6f7070"),
  "dark":  dict(org="#202221", list="#151515", page="#1a1a1a", title_list="#1c1c1c", title_page="#222222",
                ink="#b9b9b9", sec="#757575", sep="#212121", head="#6a6c6b", pill="#151515", pill_ink="#e0e0e0",
                rule="#252525", bar="#36bffa", sortbg="#3a3a3a", sortbd="#686868", sortink="#9c9c9c",
                fieldbg="#161616", fieldbd="#2e2e2e", icon="#939393", title_ink="#c6c4c7", folder="#70b7e3",
                docfill="#f5f5f5", docedge="#d7d7d7", rail_ink="#8a8c8b"),
 },
}
P["C"]["light"] = dict(P["A"]["light"]); P["C"]["dark"] = dict(P["A"]["dark"])

# G: A with the dark Organizer lifted above the list and the page, so dark chunks the way light does.
P["G"] = {"light": dict(P["A"]["light"]),
          "dark": dict(P["A"]["dark"], org="#232524", head="#7a7c7b", pill="#3a3c3b", pill_ink="#e6e6e6")}
# H: A's structure and values on warm grounds, the sans throughout.
P["H"] = {
  "light": dict(P["A"]["light"], org="#efece6", list="#fbf9f5", page="#f7f7f7", title_list="#fbf9f5", title_page="#fbf9f5",
                ink="#1c1a17", sec="#8f8a80", sep="#ebe7df", head="#8f8a80", pill="#dedad2", rule="#e4e0d8",
                sortbg="#f4f1ec", sortbd="#e5e1d9", sortink="#7c776e", fieldbg="#fbf9f5", fieldbd="#dcd7cd", icon="#8a857b",
                title_ink="#5a564f", docfill="#f6f3ee", docedge="#cfc9be"),
  "dark":  dict(P["A"]["dark"], org="#222120", list="#171614", page="#1a1a1a", title_list="#1c1b1a", title_page="#222120",
                ink="#c4bfb6", sec="#7b7670", sep="#242220", head="#6e6a64", pill="#3a3835", pill_ink="#e2ddd4", rule="#2a2826",
                sortbg="#383633", sortbd="#5e5a55", sortink="#a19b93", fieldbg="#191817", fieldbd="#302e2b", icon="#8f8a82",
                title_ink="#c4bfb6"),
}
# I: the desk — the whole pane dark on both grounds; only the page is paper.
P["I"] = {
  "light": dict(org="#1e1f1f", list="#262727", page="#f7f7f7", title_list="#262727", title_page="#fcfcfc",
                ink="#e6e6e6", sec="#8f9190", sep="#333434", head="#7c7e7d", pill="#3a3b3b", pill_ink="#ffffff",
                rule="#333434", bar="#36bffa", sortbg="#323333", sortbd="#454646", sortink="#a4a6a5",
                fieldbg="#222323", fieldbd="#3a3b3b", icon="#8f9190", title_ink="#e6e6e6", folder="#72b9e5",
                docfill="#2f3030", docedge="#8f9190", doclines="#6f7170", org_ink="#c9cbca", page_ink="#191919", page_title_ink="#545454"),
  "dark":  dict(org="#1e1f1f", list="#262727", page="#1a1a1a", title_list="#262727", title_page="#222222",
                ink="#e6e6e6", sec="#8f9190", sep="#333434", head="#7c7e7d", pill="#3a3b3b", pill_ink="#ffffff",
                rule="#333434", bar="#36bffa", sortbg="#323333", sortbd="#454646", sortink="#a4a6a5",
                fieldbg="#222323", fieldbd="#3a3b3b", icon="#8f9190", title_ink="#e6e6e6", folder="#72b9e5",
                docfill="#2f3030", docedge="#8f9190", doclines="#6f7170", org_ink="#c9cbca", page_ink="#cccccc", page_title_ink="#c6c4c7"),
}
# J: air — no separators, 80 pt rows, a tinted selection, a bare underlined Filter, more inset.
P["J"] = {
  "light": dict(P["A"]["light"], sec="#a6a6a6", head="#8a8b8b", tint="#eef6fb", sortbg=None, sortbd=None, sortink="#8a8b8b",
                fieldbd="#dcdcdc", pill="#dcdddd"),
  "dark":  dict(P["A"]["dark"], sec="#6f6f6f", head="#666868", tint="#1c2a33", sortbg=None, sortbd=None, sortink="#8a8a8a",
                fieldbd="#2e2e2e", pill="#2e302f", pill_ink="#e0e0e0"),
}
# K: mono — the pane set in the editor's monospace voice, small-caps heads, A's grounds.
P["K"] = {"light": dict(P["A"]["light"], icon="#8c8c8c"), "dark": dict(P["A"]["dark"])}

# ---------- icons (stroke SVG, 16-grid) ----------
def svg(body, w=16, h=16, stroke="currentColor", fill="none", sw=1.4):
    return (f'<svg width="{w}" height="{h}" viewBox="0 0 16 16" fill="{fill}" stroke="{stroke}" '
            f'stroke-width="{sw}" stroke-linecap="round" stroke-linejoin="round" style="flex: none;">{body}</svg>')

DOC_LINES = '<path d="M3.5 7h5M3.5 9.25h5M3.5 11.5h3.5" fill="none" stroke="{col}" stroke-width="0.9" stroke-linecap="round"></path>'

def ic_doc(fill, edge, w=12, h=15, lines=None):
    # A page with its text: iA's icon carries three short lines, so a blank page never reads as a missing icon.
    lines = lines or edge
    return (f'<svg width="{w}" height="{h}" viewBox="0 0 12 15" style="flex: none;">'
            f'<path d="M1.5 1.5h6l3 3v9h-9z" fill="{fill}" stroke="{edge}" stroke-width="1"></path>'
            f'<path d="M7.5 1.5v3h3" fill="none" stroke="{edge}" stroke-width="1"></path>'
            f'{DOC_LINES.format(col=lines)}</svg>')

def ic_doc_line(col, w=12, h=15):
    return (f'<svg width="{w}" height="{h}" viewBox="0 0 12 15" fill="none" stroke="{col}" stroke-width="1.2" '
            f'stroke-linejoin="round" style="flex: none;"><path d="M1.5 1.5h6l3 3v9h-9z"></path><path d="M7.5 1.5v3h3"></path>'
            f'{DOC_LINES.format(col=col)}</svg>')

def ic_folder(col, w=15, h=13):
    return (f'<svg width="{w}" height="{h}" viewBox="0 0 15 13" style="flex: none;">'
            f'<path d="M1 2.5a1 1 0 0 1 1-1h3.5l1.5 1.5H13a1 1 0 0 1 1 1v6.5a1 1 0 0 1-1 1H2a1 1 0 0 1-1-1z" fill="{col}"></path></svg>')

def ic_folder_line(col, w=16, h=16):
    return svg('<path d="M1.5 4a1 1 0 0 1 1-1h3.4l1.6 1.6h6a1 1 0 0 1 1 1V12a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z"></path>', w, h, col)

def ic_chev_right(col, w=8, h=12):
    return f'<svg width="{w}" height="{h}" viewBox="0 0 8 12" fill="none" stroke="{col}" stroke-width="1.25" stroke-linecap="round" stroke-linejoin="round" style="flex: none;"><path d="M2 1.5l4 4.5-4 4.5"></path></svg>'

def ic_chev_down(col, w=10, h=7):
    return f'<svg width="{w}" height="{h}" viewBox="0 0 10 7" fill="none" stroke="{col}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" style="flex: none;"><path d="M1.5 1.5L5 5l3.5-3.5"></path></svg>'

def ic_mag(col, w=13, h=13):
    return f'<svg width="{w}" height="{h}" viewBox="0 0 13 13" fill="none" stroke="{col}" stroke-width="1.5" stroke-linecap="round" style="flex: none;"><circle cx="5.5" cy="5.5" r="4"></circle><path d="M8.5 8.5l3 3"></path></svg>'

def ic_pin(col, w=16, h=16):
    return svg('<path d="M9.5 1.5l5 5-2 1-2.5 4-1.5-1.5L4 14.5 1.5 12l4.5-4.5L4.5 6l4-2.5z"></path>', w, h, col)

def ic_clock(col, w=16, h=16):
    return svg('<circle cx="8" cy="8" r="6.2"></circle><path d="M8 4.5V8l2.5 1.5"></path>', w, h, col)

def ic_sidebar(col, w=18, h=16):
    return svg('<rect x="1.5" y="2.5" width="13" height="11" rx="1.5"></rect><path d="M6 2.5v11"></path>', w, h, col)

def ic_plus(col, w=14, h=14):
    return svg('<path d="M8 3v10M3 8h10"></path>', w, h, col, sw=1.6)

def ic_lines(col, w=16, h=16):
    return svg('<path d="M2.5 4.5h11M2.5 8h11M2.5 11.5h7"></path>', w, h, col)

def ic_back(col, w=10, h=14):
    return f'<svg width="{w}" height="{h}" viewBox="0 0 10 14" fill="none" stroke="{col}" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" style="flex: none;"><path d="M7.5 1.5L2 7l5.5 5.5"></path></svg>'

def ic_fwd(col, w=10, h=14):
    return f'<svg width="{w}" height="{h}" viewBox="0 0 10 14" fill="none" stroke="{col}" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" style="flex: none;"><path d="M2.5 1.5L8 7l-5.5 5.5"></path></svg>'

# ---------- pieces ----------
def esc(s): return html.escape(s, quote=True)

def flex(style, *children, tag="div"):
    return f'<{tag} style="display: flex; {style}">' + "".join(children) + f'</{tag}>'

def text(s, style, tag="span"):
    return f'<{tag} style="{style}">{esc(s)}</{tag}>'

def file_row(c, name, date, excerpt, selected, *, bare=False, indent=0, serif=False, icons=True,
             name_px=13, sec_px=13, pitch=ROW, name_inset=40, icon_inset=22, date_right=17, sep_right=14,
             bar_w=3, bar=None, sep_from_name=True, font=None, sep=True, tint=False, icon_style="page",
             row_pad=7, tracking="-0.01em"):
    h = ROW_BARE if bare else pitch
    bar = bar or c["bar"]
    if selected and tint:
        barbox = f'<div style="position: absolute; left: 8px; right: 8px; top: 3px; bottom: 3px; border-radius: 6px; background: {c["tint"]};"></div>'
    elif selected:
        barbox = f'<div style="position: absolute; left: 0; top: 0; width: {bar_w}px; height: {h}px; background: {bar};"></div>'
    else:
        barbox = ""
    top = row_pad
    icon = ""
    if icons:
        glyph = ic_doc_line(c["sec"]) if icon_style == "line" else ic_doc(c["docfill"], c["docedge"], lines=c.get("doclines"))
        icon = (f'<div style="position: absolute; left: {icon_inset + indent}px; top: {top}px; '
                f'display: flex; align-items: center; height: 18px;">{glyph}</div>')
    nfont = font or (SERIF if serif else SANS)
    sfont = font or SANS
    name_style = (f"font-family: {nfont}; font-size: {name_px if not serif else 15}px; font-weight: 500; color: {c['ink']}; "
                  f"letter-spacing: {tracking}; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; line-height: 18px;")
    date_style = (f"font-family: {sfont}; font-size: {sec_px if not serif else 10.5}px; color: {c['sec']}; "
                  f"font-variant-numeric: tabular-nums; white-space: nowrap; line-height: 18px;"
                  + (" letter-spacing: 0.06em; text-transform: uppercase;" if serif else ""))
    namerow = (f'<div style="position: absolute; left: {name_inset + indent}px; right: {date_right}px; top: {top}px; '
               f'display: flex; justify-content: space-between; align-items: baseline; gap: 8px;">'
               f'{text(name, name_style)}{text(date, date_style)}</div>')
    exc = ""
    if not bare:
        # Two lines, cut at the second line's end: iA's excerpt ends where the line ends, with no ellipsis.
        exc = (f'<div style="position: absolute; left: {name_inset + indent}px; right: {sep_right}px; top: {top + 18}px; '
               f'font-family: {sfont}; font-size: {sec_px if not serif else 12.5}px; color: {c["sec"]}; line-height: 18px; '
               f'height: 36px; overflow: hidden;">{esc(excerpt)}</div>')
    sep_left = (name_inset + indent) if sep_from_name else icon_inset
    sepdiv = f'<div style="position: absolute; left: {sep_left}px; right: {sep_right}px; bottom: 0; height: 1px; background: {c["sep"]};"></div>' if sep else ""
    return f'<div style="position: relative; height: {h}px; flex: none;">{barbox}{icon}{namerow}{exc}{sepdiv}</div>'

def folder_row(c, name, *, open_=False, serif=False, icons=True, name_inset=40, icon_inset=22, chev_right=19, sep_right=14,
               font=None, sep=True, **_ignored):
    icon = f'<div style="position: absolute; left: {icon_inset}px; top: 9px;">{ic_folder(c["folder"])}</div>' if icons else ""
    nfont = font or (SERIF if serif else SANS)
    name_ = text(name, f"position: absolute; left: {name_inset}px; top: 7px; font-family: {nfont}; font-size: {13 if not serif else 15}px; font-weight: 500; letter-spacing: -0.01em; color: {c['ink']}; line-height: 18px;")
    chev = f'<div style="position: absolute; right: {chev_right}px; top: {10 if not open_ else 12}px;">{ic_chev_down(c["sec"]) if open_ else ic_chev_right(c["sec"])}</div>'
    sepdiv = f'<div style="position: absolute; left: {name_inset}px; right: {sep_right}px; bottom: 0; height: 1px; background: {c["sep"]};"></div>' if sep else ""
    return f'<div style="position: relative; height: {FOLDER}px; flex: none;">{icon}{name_}{chev}{sepdiv}</div>'

def sort_pill(c, label, *, capsule=True, font=None):
    f = font or SANS
    if c["sortbg"] is None:  # Quill-greys: a bare label with a chevron, no pill
        return flex(f"align-items: center; gap: 5px; height: 25px; padding: 0 2px;",
                    text(label, f"font-family: {f}; font-size: 11.5px; color: {c['sortink']}; white-space: nowrap;"),
                    ic_chev_down(c["sortink"], 8, 6))
    return flex(f"align-items: center; gap: 6px; height: 25px; padding: 0 10px; border-radius: 12.5px; "
                f"background: {c['sortbg']}; border: 1px solid {c['sortbd']}; box-sizing: border-box;",
                text(label, f"font-family: {f}; font-size: 11.5px; color: {c['sortink']}; white-space: nowrap;"),
                ic_chev_down(c["sortink"], 8, 6))

def filter_field(c, query=None, *, capsule=True, prompt="Filter", font=None, underline=False):
    r = "13px" if capsule else "4px"
    f = font or SANS
    inner = (text(query, f"font-family: {f}; font-size: 12.5px; color: {c['ink']};") if query
             else text(prompt, f"font-family: {f}; font-size: 12.5px; color: {c['sec']};"))
    if underline:
        return flex(f"align-items: center; gap: 8px; height: 26px; padding: 0 2px; border-bottom: 1px solid {c['fieldbd']}; "
                    f"box-sizing: border-box; flex-grow: 1;", ic_mag(c["icon"]), inner)
    return flex(f"align-items: center; gap: 8px; height: 26px; padding: 0 10px; border-radius: {r}; "
                f"background: {c['fieldbg']}; border: 1px solid {c['fieldbd']}; box-sizing: border-box; flex-grow: 1;",
                ic_mag(c["icon"]), inner)

def foot(c, query=None, *, capsule=True, prompt="Filter", show_rule=True, font=None, underline=False):
    rule = f"border-top: 1px solid {c['fieldbd']};" if show_rule else ""
    pad = "12px 16px 9px 16px" if underline else "12px 9px 7px 9.5px"
    return flex(f"align-items: center; padding: {pad}; {rule} flex: none;", filter_field(c, query, capsule=capsule, prompt=prompt, font=font, underline=underline))

def excerpt_of(d): return d[2]

def doc_rows(c, state, **kw):
    docs = SEARCH_DOCS if state == "search" else DOCS
    selected_none = state == "nobar"
    out = []
    if state != "search":
        out.append(folder_row(c, "Drafts", serif=kw.get("serif", False), icons=kw.get("icons", True),
                              name_inset=kw.get("name_inset", 40), icon_inset=kw.get("icon_inset", 22),
                              sep_right=kw.get("sep_right", 14), font=kw.get("font"), sep=kw.get("sep", True)))
    for name, date, exc, sel in docs:
        out.append(file_row(c, name, date, exc, sel and not selected_none, bare=(state == "bare"), **kw))
    return "".join(out)

def page_slice(c, ground, *, title_bar=True, doc_title="The storm", title_ink=None, title_bg=None, width=W - PANE, mono=MONO):
    ink = c["ink"]; tink = title_ink or c["title_ink"]
    tb = ""
    if title_bar:
        tb = (f'<div style="position: absolute; left: 0; right: 0; top: 0; height: {TITLE_H}px; background: {title_bg or c["title_page"]}; '
              f'border-bottom: 1px solid {c["rule"]}; box-sizing: border-box; display: flex; align-items: center; justify-content: center;">'
              f'{text(doc_title, f"font-family: {SANS}; font-size: 13px; color: {tink};")}'
              f'<div style="position: absolute; right: 14px; top: 18px; display: flex; gap: 4px; align-items: center;">{ic_lines(c["sec"])}{ic_chev_down(c["sec"], 8, 6)}</div>'
              f'</div>')
    body = (f'<div style="position: absolute; left: 44px; right: 0; top: {TITLE_H + 56}px; font-family: {mono}; font-size: 15px; line-height: 27px; color: {ink};">'
            f'<div style="font-weight: 700;">#&nbsp;The storm</div>'
            f'<div style="height: 27px;"></div>'
            f'<div>The gale came up the coast at four in the morning and took the roof off the boathouse.</div>'
            f'<div>By noon it had blown itself out and the gulls were back on the pilings.</div>'
            f'</div>')
    return f'<div style="position: relative; width: {width}px; height: {H}px; background: {ground}; flex: none; overflow: hidden;">{tb}{body}</div>'

# ---------- Organizer ----------
def org_head(c, label, *, style="ia", font=None):
    f = font or SANS
    if style == "caps":
        return text(label, f"font-family: {f}; font-size: 10.5px; font-weight: 600; letter-spacing: 0.08em; text-transform: uppercase; color: {c['head']}; padding: 0 16px; line-height: 16px;", "div")
    if style == "serif":
        return text(label, f"font-family: {SERIF}; font-style: italic; font-size: 13px; color: {c['head']}; padding: 0 16px; line-height: 16px;", "div")
    if style == "smallcaps":
        return text(label, f"font-family: {f}; font-size: 11px; font-weight: 500; letter-spacing: 0.1em; text-transform: uppercase; color: {c['head']}; padding: 0 16px; line-height: 16px;", "div")
    if style == "air":
        return text(label, f"font-family: {f}; font-size: 11.5px; font-weight: 600; color: {c['head']}; padding: 0 20px; line-height: 16px;", "div")
    # iA: the head's text sits over the pill's icon, 15 px in (the oracle's 16.5 pt, the pill 10 pt in)
    return text(label, f"font-family: {f}; font-size: 11px; font-weight: 700; letter-spacing: -0.005em; color: {c['head']}; padding: 0 15px; line-height: 16px;", "div")

def org_row(c, label, icon, *, current=False, style="ia", empty_prose=False, font=None):
    f = font or SANS
    if empty_prose:
        pad = 20 if style == "air" else 15 if style == "ia" else 16
        return text(label, f"font-family: {f if style != 'serif' else SERIF}; font-size: 12px; color: {c['sec']}; padding: 0 {pad}px; line-height: 16px; margin: 6px 0 0;", "div")
    if style in ("ia", "smallcaps", "air") and c.get("pill"):
        bg = f"background: {c['pill']};" if current else ""
        ink = c["pill_ink"] if current else c.get("org_ink", c["ink"])
        return flex(f"align-items: center; gap: 6px; height: 32px; margin: 0 8px; padding: 0 7px; border-radius: 5.5px; {bg} flex: none;",
                    icon(ink), text(label, f"font-family: {f}; font-size: 12.5px; font-weight: {600 if current else 400}; letter-spacing: -0.01em; color: {ink}; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-width: 0;"))
    # quiet: weight only
    ink = c["ink"] if current else c["sec"]
    font_ = SERIF if style == "serif" else f
    return flex(f"align-items: center; gap: 7px; height: 26px; padding: 0 16px; flex: none;",
                icon(ink), text(label, f"font-family: {font_}; font-size: {13 if style != 'serif' else 14}px; font-weight: {600 if current else 400}; color: {ink}; white-space: nowrap;"))

def organizer(c, state, *, style="ia", width=ORG, font=None):
    heads = ("Locations", "Pinned", "Recents")
    parts = [f'<div style="height: {8.5 + 2}px; flex: none;"></div>']
    parts.append(org_head(c, heads[0], style=style, font=font)); parts.append('<div style="height: 8.5px; flex: none;"></div>')
    for i, loc in enumerate(LOCATIONS):
        parts.append(org_row(c, loc, lambda col: ic_folder_line(col, 15, 15), current=(i == 0), style=style, font=font))
    parts.append('<div style="height: 22px; flex: none;"></div>')
    parts.append(org_head(c, heads[1], style=style, font=font)); parts.append('<div style="height: 8.5px; flex: none;"></div>')
    if state == "pinned":
        for p in PINNED:
            parts.append(org_row(c, p, lambda col: ic_doc_line(col, 11, 14), style=style, font=font))
    else:
        parts.append(org_row(c, "Pin a Document or folder to keep it here", None, style=style, empty_prose=True, font=font))
    parts.append('<div style="height: 22px; flex: none;"></div>')
    parts.append(org_head(c, heads[2], style=style, font=font)); parts.append('<div style="height: 8.5px; flex: none;"></div>')
    parts.append(org_row(c, "Last opened", lambda col: ic_clock(col, 15, 15), style=style, font=font))
    return f'<div style="width: {width}px; height: {H}px; background: {c["org"]}; display: flex; flex-direction: column; flex: none; box-sizing: border-box; padding-top: {TITLE_H}px;">' + "".join(parts) + '</div>'

def rail(c, state):
    def slot(icon, current=False, label=None):
        bg = f"background: {c['pill']};" if current else ""
        ink = c["ink"] if current else c["rail_ink"]
        lab = text(label, f"font-family: {SANS}; font-size: 9.5px; color: {ink}; line-height: 11px; text-align: center;") if label else ""
        return flex(f"flex-direction: column; align-items: center; justify-content: center; gap: 3px; width: 44px; height: 44px; margin: 0 6px; border-radius: 8px; {bg} flex: none;", icon(ink), lab)
    parts = [f'<div style="height: 10px; flex: none;"></div>']
    for i, loc in enumerate(LOCATIONS):
        parts.append(slot(lambda col: ic_folder_line(col, 18, 18), current=(i == 0), label=loc[:1]))
    parts.append('<div style="height: 16px; flex: none;"></div>')
    parts.append(f'<div style="height: 1px; margin: 0 14px; background: {c["sep"]}; flex: none;"></div>')
    parts.append('<div style="height: 16px; flex: none;"></div>')
    parts.append(slot(lambda col: ic_pin(col, 18, 18)))
    parts.append(slot(lambda col: ic_clock(col, 18, 18)))
    return f'<div style="width: 56px; height: {H}px; background: {c["org"]}; display: flex; flex-direction: column; flex: none; box-sizing: border-box; padding-top: {TITLE_H}px;">' + "".join(parts) + '</div>'

# ---------- the pane's title row over the list ----------
def list_title(c, label, *, width, bg, icon_left=True, plus=True, chevron=False, serif=False, ink=None, font=None):
    ink = ink or c["title_ink"]
    left = f'<div style="display: flex; align-items: center; margin-right: 10px;">{ic_back(c["sec"])}</div>' if icon_left else ""
    f = font or (SERIF if serif else SANS)
    lab = flex("align-items: center; gap: 6px; flex-grow: 1; min-width: 0;",
               text(label, f"font-family: {f}; font-size: {14 if not serif else 16}px; font-weight: {700 if not serif else 600}; letter-spacing: -0.01em; color: {ink}; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;"),
               ic_chev_down(c["sec"], 9, 6) if chevron else "")
    # The New control: iA's is a bordered pill, but at this scale a border reads heavy, so the two glyphs stand
    # bare in the secondary grey with a hairline between them.
    right = flex(f"align-items: center; gap: 7px; height: 26px; padding: 0 4px; flex: none;",
                 ic_plus(c["sec"], 13, 13),
                 f'<div style="width: 1px; height: 12px; background: {c["sep"]}; flex: none;"></div>',
                 ic_chev_down(c["sec"], 8, 6)) if plus else ""
    return flex(f"align-items: center; width: {width}px; height: {TITLE_H}px; padding: 0 12px 0 16px; background: {bg}; flex: none; box-sizing: border-box;", left, lab, right)

def toggle_cell(c, width, bg):
    return flex(f"align-items: center; justify-content: flex-end; height: {TITLE_H}px; width: {width}px; padding-right: 8px; background: {bg}; flex: none; box-sizing: border-box;",
                ic_sidebar(c["sec"]))

# ---------- directions ----------
def sort_label(state):
    return "Sort by Relevance" if state == "search" else "Sort by Date Modified"

def two_column(c, state, *, serif=False, icons=True, org_style="ia", capsule=True, query=None, rail_mode=False,
               font=None, sep=True, tint=False, pitch=None, underline=False, icon_style="page", inset=0, row_pad=None):
    org_w = 56 if rail_mode else ORG
    list_w = PANE - org_w
    query = "sea" if state == "search" else None
    left = rail(c, state) if rail_mode else organizer(c, state, style=org_style, font=font)
    # title row spans the Organizer (toggle) and the list (Location title) — drawn over the columns
    title = flex(f"position: absolute; left: 0; top: 0; width: {PANE}px; height: {TITLE_H}px;",
                 toggle_cell(c, org_w, c["org"]),
                 list_title(c, LOCATIONS[0], width=list_w, bg=c["title_list"], serif=serif, font=font))
    bars = flex(f"align-items: center; height: {SORT_H}px; padding: 0 {8 + inset}px; background: {c['title_list']}; border-bottom: 1px solid {c['rule']}; flex: none; box-sizing: border-box;", sort_pill(c, sort_label(state), capsule=capsule, font=font))
    pitch = pitch or (76 if serif else ROW)
    rowkw = dict(serif=serif, icons=icons, name_inset=(40 if icons else 22) + inset, icon_inset=22 + inset, sep_right=14 + inset,
                 pitch=pitch, sep_from_name=icons, font=font, sep=sep, tint=tint, icon_style=icon_style)
    if row_pad is not None:
        rowkw["row_pad"] = row_pad
    rows = doc_rows(c, state, **rowkw)
    listcol = (f'<div style="width: {list_w}px; height: {H}px; background: {c["list"]}; display: flex; flex-direction: column; flex: none; box-sizing: border-box; padding-top: {TITLE_H}px;">'
               f'{bars}<div style="height: 8px; flex: none;"></div>'
               f'<div style="display: flex; flex-direction: column; flex-grow: 1; overflow: hidden;">{rows}</div>'
               f'{foot(c, query, capsule=capsule, font=font, underline=underline)}</div>')
    pane = f'<div style="position: relative; width: {PANE}px; height: {H}px; display: flex; flex: none;">{left}{listcol}{title}</div>'
    return pane

def one_column(c, state):
    query = "sea" if state == "search" else None
    title = flex(f"position: absolute; left: 0; top: 0; width: {PANE}px; height: {TITLE_H}px;",
                 toggle_cell(c, 44, c["list"]),
                 list_title(c, LOCATIONS[0], width=PANE - 44, bg=c["list"], icon_left=False, chevron=True))
    bars = "" if state == "bare" else flex(f"align-items: center; height: {SORT_H}px; padding: 0 8px; background: {c['list']}; border-bottom: 1px solid {c['rule']}; flex: none; box-sizing: border-box;", sort_pill(c, sort_label(state)))
    sections = []
    def head(label):
        return text(label, f"font-family: {SANS}; font-size: 11px; font-weight: 700; color: {c['head']}; padding: 10px 22px 6px; line-height: 16px;", "div")
    if state != "search":
        sections.append(head("Pinned"))
        if state == "pinned":
            for p in PINNED:
                sections.append(file_row(c, p, "Mar 2", "", False, bare=True))
        else:
            sections.append(text("Pin a Document or folder to keep it here", f"font-family: {SANS}; font-size: 12px; color: {c['sec']}; padding: 0 22px 8px; line-height: 16px;", "div"))
        sections.append('<div style="height: 10px; flex: none;"></div>')
    sections.append(doc_rows(c, state))
    listcol = (f'<div style="width: {PANE}px; height: {H}px; background: {c["list"]}; display: flex; flex-direction: column; flex: none; box-sizing: border-box; padding-top: {TITLE_H}px;">'
               f'{bars}<div style="display: flex; flex-direction: column; flex-grow: 1; overflow: hidden;">{"".join(sections)}</div>'
               f'{"" if state == "bare" else foot(c, query)}</div>')
    return f'<div style="position: relative; width: {PANE}px; height: {H}px; display: flex; flex: none;">{listcol}{title}</div>'

def artboard(direction, state, ground):
    c = P[direction][ground]
    fonts = '<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&amp;family=Source+Serif+4:ital,wght@0,400;0,500;0,600;1,400&amp;family=JetBrains+Mono:wght@400;500;600&amp;display=swap">'
    if direction == "A":
        pane = two_column(c, state)
    elif direction == "B":
        pane = two_column(c, state, org_style="caps", capsule=False)
    elif direction == "C":
        pane = one_column(c, state)
    elif direction == "D":
        pane = two_column(c, state)
    elif direction == "E":
        pane = two_column(c, state, serif=True, icons=False, org_style="serif", capsule=False)
    elif direction == "F":
        pane = two_column(c, state, rail_mode=True)
    elif direction in ("G", "H"):
        pane = two_column(c, state)
    elif direction == "I":
        pane = two_column(c, state)
    elif direction == "J":
        pane = two_column(c, state, org_style="air", capsule=False, sep=False, tint=True, pitch=80, underline=True, inset=6, row_pad=13)
    elif direction == "K":
        pane = two_column(c, state, org_style="smallcaps", font=MONO, icon_style="line")
    divider = f'<div style="width: 1px; height: {H}px; background: {c["rule"]}; flex: none;"></div>' if direction in ("B",) else ""
    pc = dict(c, ink=c.get("page_ink", c["ink"]), title_ink=c.get("page_title_ink", c["title_ink"]),
              rule=("#dddddd" if ground == "light" else "#292929") if direction == "I" else c["rule"],
              sec=("#8c8c8c" if ground == "light" else "#7e7e7e") if direction == "I" else c["sec"])
    page = page_slice(pc, c["page"], width=W - PANE - (1 if divider else 0))
    body = f'<div style="display: flex; width: {W}px; height: {H}px; overflow: hidden; background: {c["page"]};">{pane}{divider}{page}</div>'
    return f'''<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
  {fonts}
  <style>
    body {{ margin: 0; font-family: {SANS}; -webkit-font-smoothing: antialiased; }}
    a {{ color: #00bfff; }} a:hover {{ color: #0099cc; }}
  </style>
</helmet>
{body}
</x-dc>
</body>
</html>
'''

DIRS = [
    ("A", "iA, faithfully", "Two columns on three grounds at every measured value. Motivation: the quality bar itself, and the drag range, grounds, bar and foot field all belong to this shape. Tradeoff: it is a Mac look, and the two-section Organizer is sparse where iA's four are not."),
    ("B", "iA's structure in Quill's greys", "Two columns, two grounds: the Organizer stepped off the paper, the list on the paper, hierarchy by weight and spacing, Quill's accent and chrome grey. Motivation: the structure without importing the look. Tradeoff: the list still sits on the page's ground, which is the round-11 gap."),
    ("C", "One column with a Location head", "One list on its own ground, the current Location as a switcher head, Pinned as a section, the oracle's rows and foot. Motivation: the honest control; if this reads as well, the Organizer is not earning its width. Tradeoff: Locations and Pinned are a menu away, not in view."),
    ("D", "A drawer", "Two columns where the Organizer is dark furniture on light and lighter than the paper on dark, so the pane reads as pulled out beside the page. Motivation: a bolder identity that is not a Mac look. Tradeoff: a dark column beside a light page is loud at rest, and it needs its own ink tier."),
    ("E", "Editorial", "A manuscript index: serif names in Source Serif 4, small-caps dates, no icons, hairlines the row's full width, italic section heads, warm grounds. Motivation: Quill's own voice, from the Template family it already ships. Tradeoff: a second face in the chrome, and warm grounds fight a writer's palette file."),
    ("F", "A rail", "Locations, Pinned and Recents as a 56 px icon rail with the list taking the width. Motivation: the Organizer's job in a fifth of its width. Tradeoff: Locations become initials with no names in view, and the empty-Pinned prose has nowhere to live."),
    ("G", "A, dark Organizer lifted", "A on light, unchanged. On dark the Organizer steps lighter (#232524) than both the list (#151515) and the page (#1a1a1a), so the pane chunks on dark the way A's light does and iA's own dark pane does. Motivation: round 3 — A's dark was less distinct than its light. Tradeoff: the Organizer is the brightest ground in a dark window, so it draws the eye at rest."),
    ("H", "A, warm", "A's structure and every measured value on warm grounds — the paper stays #f7f7f7, the pane goes cream — with warm greys and the sans throughout; E's warmth without its serif. Motivation: round 3 liked the warmth and not the face. Tradeoff: a warm pane beside a neutral page shows the seam, and a writer's palette file has to carry the warmth or lose it."),
    ("I", "The desk", "The whole pane is dark on both grounds — Organizer #1e1f1f, list #262727 — and only the page is paper. The pane is the desk, the document is the sheet on it. Motivation: the strongest chunking on offer, and one pane for both themes. Tradeoff: on light the window is half dark, which is loud, and the pane's ink can never follow the page's palette."),
    ("J", "Air", "No separators, 80 pt rows, the selected row a soft tint rather than a bar, secondary text a step lighter, more inset, a bare underlined Filter. Motivation: whether polish is space rather than lines. Tradeoff: fewer rows in view, and a tint does what iA's bar deliberately does not — fill the row."),
    ("K", "Mono", "The pane set in the editor's monospace face with letterspaced small-caps heads and line icons, on A's grounds and pitch. Motivation: chrome and page in one voice, the way a typewriter's is. Tradeoff: monospace is wide, so names and excerpts hold fewer characters, and the pane stops reading as furniture."),
]
STATES = [("Rest", "rest"), ("Pinned", "pinned"), ("Search", "search")]
EXTRA = [("Bare", "bare"), ("NoBar", "nobar")]

def main():
    boards, notes = [], []
    colw, rowh = W + 80, H + 140
    for r, (d, name, note) in enumerate(DIRS):
        y = r * rowh
        notes.append({"id": f"dir-{d}", "x": -300, "y": y + 20, "w": 260, "text": f"{d}. {name}\n\n{note}"})
        col = 0
        cells = [(s, g) for s, _ in [(st, 0) for st in STATES] for g in ("light", "dark")]
        if d in ("A", "B"):
            cells += [(e, "light") for e in EXTRA]
        for (label, state), g in cells:
            stem = "Main" if (d == "A" and state == "rest" and g == "light") else f"{d}{label}{g.capitalize()}"
            fn = f"{stem}.dc.html"
            with open(os.path.join(OUT, fn), "w") as f:
                f.write(artboard(d, state, g))
            boards.append({"file": fn, "x": col * colw, "y": y, "w": W, "h": H,
                           "title": f"{d} {name} · {label} · {g}"})
            col += 1
    canvas = {"artboards": boards, "annotations": notes, "launch": {"view": "canvas"}}
    with open(os.path.join(OUT, "canvas.json"), "w") as f:
        json.dump(canvas, f, indent=1)
    print(len(boards), "artboards")

if __name__ == "__main__":
    main()
