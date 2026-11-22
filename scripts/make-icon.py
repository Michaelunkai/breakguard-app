"""Generate breakguard.ico: crescent moon inside a shield, sizes 16/32/48/256.

Usage: python scripts/make-icon.py [output.ico]
Requires Pillow.
"""
import sys
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw

SIZES = [16, 32, 48, 256]
BASE = 1024  # supersample canvas, downscaled to 256 for the largest icon
SHIELD_FILL = (30, 58, 138, 255)
MOON_FILL = (250, 204, 21, 255)


def shield_points(s):
    return [
        (s * 0.50, s * 0.04),
        (s * 0.88, s * 0.16),
        (s * 0.88, s * 0.50),
        (s * 0.50, s * 0.96),
        (s * 0.12, s * 0.50),
        (s * 0.12, s * 0.16),
    ]


def render_master(s=BASE):
    img = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    ImageDraw.Draw(img).polygon(shield_points(s), fill=SHIELD_FILL)

    # Crescent: big disc minus offset disc, built as an alpha mask.
    big = Image.new("L", (s, s), 0)
    r = s * 0.24
    cx, cy = s * 0.50, s * 0.50
    ImageDraw.Draw(big).ellipse((cx - r, cy - r, cx + r, cy + r), fill=255)
    cut = Image.new("L", (s, s), 0)
    off = s * 0.09
    ImageDraw.Draw(cut).ellipse(
        (cx - r + off, cy - r - off, cx + r + off, cy + r - off), fill=255
    )
    mask = ImageChops.subtract(big, cut)

    moon = Image.new("RGBA", (s, s), MOON_FILL)
    moon.putalpha(mask)
    img.alpha_composite(moon)
    return img


def main():
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent / "breakguard.ico"
    master = render_master().resize((256, 256), Image.LANCZOS)
    master.save(out, format="ICO", sizes=[(n, n) for n in SIZES])
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
