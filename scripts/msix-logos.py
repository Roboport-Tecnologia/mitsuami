#!/usr/bin/env python3
"""Renders an app's MSIX logos (its msix/Assets) from its logo SVGs:

  scripts/msix-logos.py examples/showcase/msix
  scripts/msix-logos.py crates/mitsuami/examples/files/msix

The logo is `logo.svg` in that folder, or `logo-light.svg` and
`logo-dark.svg` when it differs on light and dark backgrounds. A logo may
paint with currentColor where it shows the background through, as the
mitsuami mark's cuts at its crossings do.

- Plated logos (tiles, the Store logo, and the list and taskbar icons
  without an `altform`) put the light logo on the brand's tile.
- Unplated ones (`altform-unplated` for dark taskbars, `-lightunplated` for
  light ones) are the logo alone, on transparency. Each is rendered on
  white and on black, its currentColor matching, and the difference gives
  every pixel's opacity, so the cuts are clear and not the tile's colour.

Needs rsvg-convert (librsvg) and ImageMagick's `magick`. After changing
the logos, scripts\\package-msix.ps1 indexes them again (resources.pri).
"""
import re
import subprocess
import sys
import tempfile
from pathlib import Path

TILE = "#F3F4F8"
TILE_BORDER = "#D9DCE6"
TARGET_SIZES = [16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 256]
SCALES = [100, 125, 150, 200, 400]


def inner(svg):
    """The logo's viewBox and contents, to nest it in another SVG."""
    view_box = re.search(r'viewBox="([^"]+)"', svg).group(1)
    body = re.sub(r"^.*?<svg[^>]*>|</svg>\s*$", "", svg, flags=re.S)
    return view_box, body


def logo_at(svg, x, y, size, color):
    view_box, body = inner(svg)
    return (f'<svg x="{x}" y="{y}" width="{size}" height="{size}" viewBox="{view_box}" '
            f'color="{color}">{body}</svg>')


def render(svg, width, height, out):
    with tempfile.NamedTemporaryFile("w", suffix=".svg") as f:
        f.write(svg)
        f.flush()
        subprocess.run(["rsvg-convert", "-w", str(width), "-h", str(height), "-o", str(out), f.name],
                       check=True)


def plated(logo, width, height, out):
    # The tile's corners and the logo's share of it are the old logos'.
    side = min(width, height)
    radius = side * 0.18
    size = side * 0.68
    border = max(1.0, side / 150)
    svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
           f'viewBox="0 0 {width} {height}">'
           f'<rect x="{border / 2}" y="{border / 2}" width="{width - border}" height="{height - border}" '
           f'rx="{radius}" fill="{TILE}" stroke="{TILE_BORDER}" stroke-width="{border}"/>'
           f'{logo_at(logo, (width - size) / 2, (height - size) / 2, size, TILE)}</svg>')
    render(svg, width, height, out)


def unplated(logo, size, out, work):
    shots = {}
    for name, color in [("white", "#FFFFFF"), ("black", "#000000")]:
        svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" '
               f'viewBox="0 0 {size} {size}"><rect width="{size}" height="{size}" fill="{color}"/>'
               f'{logo_at(logo, 0, 0, size, color)}</svg>')
        shots[name] = work / f"{name}.png"
        render(svg, size, size, shots[name])
    # Opacity is 1 - (on white - on black); the colour is on black / opacity.
    alpha = work / "alpha.png"
    subprocess.run(["magick", shots["white"], shots["black"], "-compose", "difference", "-composite",
                    "-colorspace", "gray", "-negate", alpha], check=True)
    subprocess.run(["magick", shots["black"], alpha, "-compose", "DivideSrc", "-composite",
                    alpha, "-alpha", "off", "-compose", "CopyOpacity", "-composite", out], check=True)


def main():
    folder = Path(sys.argv[1])
    single = folder / "logo.svg"
    light = (folder / "logo-light.svg" if not single.exists() else single).read_text()
    dark = (folder / "logo-dark.svg" if not single.exists() else single).read_text()
    assets = folder / "Assets"
    assets.mkdir(exist_ok=True)
    for old in assets.glob("*.png"):
        old.unlink()

    for scale in SCALES:
        k = scale / 100
        plated(light, round(44 * k), round(44 * k), assets / f"Square44x44Logo.scale-{scale}.png")
        plated(light, round(150 * k), round(150 * k), assets / f"Square150x150Logo.scale-{scale}.png")
        plated(light, round(310 * k), round(150 * k), assets / f"Wide310x150Logo.scale-{scale}.png")
        plated(light, round(50 * k), round(50 * k), assets / f"StoreLogo.scale-{scale}.png")
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        for size in TARGET_SIZES:
            plated(light, size, size, assets / f"Square44x44Logo.targetsize-{size}.png")
            unplated(dark, size, assets / f"Square44x44Logo.targetsize-{size}_altform-unplated.png", work)
            unplated(light, size, assets / f"Square44x44Logo.targetsize-{size}_altform-lightunplated.png", work)
    print(f"{len(list(assets.glob('*.png')))} logos in {assets}")


if __name__ == "__main__":
    main()
