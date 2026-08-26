#!/usr/bin/env python3
"""Regularise iA Writer font advance widths onto the writing grid.

The editor draws every glyph twice: once in the hidden <textarea> (always
Regular) and once in #mirror (Regular / Italic / Bold as the markup says). The
two only line up if a character keeps the same advance in every style. Three
places in the shipped iA fonts break that; this rewrites those hmtx entries.

Usage: fontgrid.py <in.ttf> <out.ttf> <char>=<units> [...]
"""
import sys, struct

def tables(d):
    n = struct.unpack('>H', d[4:6])[0]
    t = {}
    for i in range(n):
        off = 12 + 16*i
        tag = d[off:off+4].decode('latin1')
        cs, o, l = struct.unpack('>III', d[off+4:off+16])
        t[tag] = (off, cs, o, l)
    return t

def cmap_lookup(d, t):
    off = t['cmap'][2]
    n = struct.unpack('>H', d[off+2:off+4])[0]
    best = None
    for i in range(n):
        pid, eid, so = struct.unpack('>HHI', d[off+4+8*i:off+12+8*i])
        fmt = struct.unpack('>H', d[off+so:off+so+2])[0]
        if fmt == 4 and (pid, eid) in ((3, 1), (0, 3), (0, 4)):
            best = off + so
        if fmt == 12 and best is None:
            best = off + so
    assert best, 'no usable cmap'
    fmt = struct.unpack('>H', d[best:best+2])[0]
    m = {}
    if fmt == 4:
        segX2 = struct.unpack('>H', d[best+6:best+8])[0]
        seg = segX2 // 2
        p = best + 14
        end = struct.unpack('>%dH' % seg, d[p:p+segX2]); p += segX2 + 2
        start = struct.unpack('>%dH' % seg, d[p:p+segX2]); p += segX2
        delta = struct.unpack('>%dh' % seg, d[p:p+segX2]); p += segX2
        ro_at = p
        ro = struct.unpack('>%dH' % seg, d[p:p+segX2])
        for i in range(seg):
            for c in range(start[i], min(end[i], 0xFFFF) + 1):
                if ro[i] == 0:
                    g = (c + delta[i]) & 0xFFFF
                else:
                    gp = ro_at + 2*i + ro[i] + 2*(c - start[i])
                    g = struct.unpack('>H', d[gp:gp+2])[0]
                    if g: g = (g + delta[i]) & 0xFFFF
                if g: m[c] = g
    else:
        ng = struct.unpack('>I', d[best+12:best+16])[0]
        for i in range(ng):
            s, e, gi = struct.unpack('>III', d[best+16+12*i:best+28+12*i])
            for c in range(s, e+1): m[c] = gi + (c - s)
    return m

def checksum(b):
    if len(b) % 4: b = b + b'\0' * (4 - len(b) % 4)
    return sum(struct.unpack('>%dI' % (len(b)//4), b)) & 0xFFFFFFFF

def freeze_gvar(d, t, gid):
    """Drop a glyph's variation data so its advance (and outline) stay at the
    default instance. Only safe for glyphs with no contours, e.g. the space."""
    if 'gvar' not in t: return False
    off = t['gvar'][2]
    axisCount, sharedCount = struct.unpack('>HH', d[off+4:off+8])
    glyphCount, flags = struct.unpack('>HH', d[off+12:off+16])
    if gid >= glyphCount: return False
    long_off = flags & 1
    base = off + 20
    if long_off:
        a = base + 4*gid
        lo, hi = struct.unpack('>II', d[a:a+8])
        struct.pack_into('>I', d, a+4, lo)
    else:
        a = base + 2*gid
        lo, hi = struct.unpack('>HH', d[a:a+4])
        struct.pack_into('>H', d, a+2, lo)
    return True

def main():
    src, dst = sys.argv[1], sys.argv[2]
    edits = {}
    freeze = []
    for a in sys.argv[3:]:
        if a.startswith('freeze:'):
            freeze.append(a.split(':', 1)[1]); continue
        ch, v = a.split('=')
        ch = chr(int(ch[2:], 16)) if ch.startswith('U+') else ch
        edits[ch] = int(v)
    d = bytearray(open(src, 'rb').read())
    t = tables(d)
    if 'HVAR' in t:
        print('  ! HVAR present — advances may vary per instance', file=sys.stderr)
    nhm = struct.unpack('>H', d[t['hhea'][2]+34:t['hhea'][2]+36])[0]
    cm = cmap_lookup(d, t)
    hoff = t['hmtx'][2]
    changed = []
    for ch, want in edits.items():
        g = cm.get(ord(ch))
        if g is None: print('  ! no glyph for %r' % ch, file=sys.stderr); continue
        if g >= nhm: print('  ! glyph %d outside hmtx metrics' % g, file=sys.stderr); continue
        p = hoff + 4*g
        old = struct.unpack('>H', d[p:p+2])[0]
        if old == want: continue
        struct.pack_into('>H', d, p, want)
        changed.append((ch, g, old, want))
    for ch in freeze:
        ch = chr(int(ch[2:], 16)) if ch.startswith('U+') else ch
        g = cm.get(ord(ch))
        if g is not None and freeze_gvar(d, t, g): changed.append((ch, g, 'gvar', 'frozen'))
    # rewrite table checksums, then head.checkSumAdjustment
    for tag, (rec, cs, off, ln) in t.items():
        struct.pack_into('>I', d, rec+4, checksum(bytes(d[off:off+ln])))
    ho = t['head'][2]
    struct.pack_into('>I', d, ho+8, 0)
    struct.pack_into('>I', d, t['head'][0]+4, checksum(bytes(d[ho:ho+t['head'][3]])))
    total = checksum(bytes(d))
    struct.pack_into('>I', d, ho+8, (0xB1B0AFBA - total) & 0xFFFFFFFF)
    struct.pack_into('>I', d, t['head'][0]+4, checksum(bytes(d[ho:ho+t['head'][3]])))
    open(dst, 'wb').write(bytes(d))
    print('%s -> %s  %s' % (src.split('/')[-1], dst.split('/')[-1],
          ', '.join('%r %s->%s' % (c, o, w) for c, g, o, w in changed) or 'no change'))

main()
