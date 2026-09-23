#!/usr/bin/env python3
"""Verify the 2026-09-09 capture manifest and re-read its selection measurements.

Run from the repository root with the rig's Pillow/numpy Python. Reads only files;
no Mac UI, display, or private app state is needed. Output coordinates are device px.
"""
import hashlib
import json
from pathlib import Path

import numpy as np
from PIL import Image

import colour
import fast


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    manifest = json.loads(Path('dev/ref/ia/mac-native/capture-2026-09-09.json').read_text())
    profile = Path('dev/ref/ia/mac-native/rig/display.icc').read_bytes()
    for name, item in manifest['captures'].items():
        path = Path(item['normalized_file'])
        raw = Path(item['raw_file'])
        assert digest(path) == item['sha256'], name
        assert digest(raw) == item['raw_sha256'], name
        im = Image.open(path)
        assert list(im.size) == item['size_px'], name
        assert im.size == tuple(2 * n for n in item['region_pt'][2:]), name
        assert Image.open(raw).size == im.size, name
        assert im.info['icc_profile'] == profile, name
        if item['validation'] == 'colour.check pass':
            colour.check(str(path), 'light' if '-light-' in name else 'dark')
    print(f"capture manifest: pass ({len(manifest['captures'])} originals and normalised frames)")
    base = Path('dev/ref/ia/shots/mac-native')
    for suffix in ['05-cells', '10-cells', '20-cells', '40-cells', 'select-all']:
        path = base / f'mac-native-18-light-page-narrow-{suffix}.png'
        print(suffix, json.dumps(fast.fill_box(path, (209, 236, 248), tol=0)))
    for step in [0, 5, 13]:
        path = base / f'mac-native-18-dark-page-top-empty-step-{step:02d}.png'
        print(f'empty step {step}', json.dumps(fast.caret_box(path)))
    for ground in ['light', 'dark']:
        a = np.asarray(Image.open(base / f'mac-native-21-{ground}-syntax-all.png').convert('RGB'))
        for role, value in manifest['syntax_colours'][ground].items():
            rgb = tuple(bytes.fromhex(value['normalized'][1:]))
            count = int((a == rgb).all(axis=2).sum())
            assert count == value['solid_pixels'], (ground, role)
    print('syntax solid-colour counts: pass')


if __name__ == '__main__':
    main()
