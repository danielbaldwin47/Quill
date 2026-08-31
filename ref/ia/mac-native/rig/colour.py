#!/usr/bin/env python3
"""Put a capture back into the profile the committed captures were taken in.

`screencapture` hands back the display's own values and tags them with the
display's ICC profile. Every capture up to state 16 was taken while the built-in
display carried its stock profile; the display now carries a calibration profile
(DisplayCAL, "Display #1 2025-12-15 …"), and the same pixels come back as
`#252525` paper and `#d1d1d1` ink rather than the `#1a1a1a` and `#cccccc` the
rows are written in.

The fix is a colour conversion, not a system change — calibrating the display is
the writer's. The stock profile is kept beside this file as `display.icc`, taken
out of `mac-native-14-dark-markup.png`, and a capture is converted from whatever
profile it carries into it. The conversion is checked, not assumed: `check()`
reads a committed capture back and fails unless the paper and the body ink land
on the values the § 4.2 rows hold.
"""
import io
import os

from PIL import Image, ImageCms

REF = os.path.join(os.path.dirname(os.path.abspath(__file__)), "display.icc")


def ref_profile():
    with open(REF, "rb") as f:
        return ImageCms.ImageCmsProfile(io.BytesIO(f.read()))


def normalise(png, out=None):
    """Rewrite a capture in the reference display profile. In place by default."""
    im = Image.open(png)
    icc = im.info.get("icc_profile")
    rgb = im.convert("RGB")
    if icc:
        src = ImageCms.ImageCmsProfile(io.BytesIO(icc))
        # Relative colorimetric: the two profiles describe one display, so the
        # reading wanted is "the same light, in the other profile's numbers".
        t = ImageCms.buildTransform(src, ref_profile(), "RGB", "RGB",
                                    renderingIntent=1)
        rgb = ImageCms.applyTransform(rgb, t)
    with open(REF, "rb") as f:
        rgb.save(out or png, icc_profile=f.read())
    return out or png


PAPER = {"dark": (0x1a, 0x1a, 0x1a), "light": (0xf7, 0xf7, 0xf7)}
INK = {"dark": (0xcc, 0xcc, 0xcc), "light": (0x19, 0x19, 0x19)}


def check(png, ground):
    """A normalised capture still reads the paper and ink the § 4.2 rows hold."""
    im = Image.open(png).convert("RGB")
    seen = set(im.getdata())
    for role, want in (("paper", PAPER[ground]), ("ink", INK[ground])):
        if want not in seen:
            raise SystemExit(f"{png}: no {role} pixel at {want} — the profile "
                             f"conversion has drifted off the rows")


if __name__ == "__main__":
    import sys
    for p in sys.argv[1:]:
        normalise(p)
        check(p, "light" if "light" in p else "dark")
        print("normalised", p)
