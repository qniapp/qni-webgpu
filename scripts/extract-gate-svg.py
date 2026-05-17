#!/usr/bin/env python3
"""Geist フォントから単一字ゲートのグリフアウトラインを抽出し、
`apps/egui-web/assets/icons/<char>.svg` と同名 PNG に書き出す。

design-system-architecture.html の Tier A 提案 (SVG アイコンを Rust と
TypeScript で 1 ソース共有) に従い、ゲートアイコンの文字部分を
フォント由来の SVG ファイルとして固定化するための補助スクリプト。

使い方:
    python3 scripts/extract-gate-svg.py H
    python3 scripts/extract-gate-svg.py H X Y Z   # 複数

設定:
    - 単一字ゲートは Geist Regular 400 (gate_glyphs.rs の
      `GATE_LABEL_LIGHT_FAMILY` 規約に合わせる)
    - viewBox は 48×48 (apps/egui-web/src/icons.rs の VIEWBOX)
    - グリフ高さは viewBox の 0.62 倍 (`base_label_font_px` の単一字レシオ)

依存:
    pip install --user --break-system-packages fonttools
    sudo apt install librsvg2-bin  # rsvg-convert。無い場合は ImageMagick の magick を使う。
"""

import os
import shutil
import subprocess
import sys
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
FONTS_DIR = os.path.join(REPO_ROOT, "apps/egui-web/assets")
OUT_DIR = os.path.join(REPO_ROOT, "apps/egui-web/assets/icons")
VIEWBOX = 48.0
RASTER_SIZE = 128
GLYPH_RATIO = 0.62  # gate_glyphs.rs::base_label_font_px の単一字レシオと一致

# gate_glyphs.rs::base_label_family の規約に合わせる:
#   - 単一字 (H / Y / Z / S / T) → Geist Regular 400
#   - "+" (X ゲートの本体) → Geist Medium 500 (2 ストロークだけだと Regular だと細く見えるため)
#   - 複数字 / 太字想定 → Geist Bold 700 (本スクリプトでは未対応)
WEIGHT_FOR_CHAR = {
    "+": "Medium",
}
DEFAULT_WEIGHT = "Regular"

# ファイル名に使えない文字をマップ。
FILENAME_FOR_CHAR = {
    "+": "plus",
}


def extract(char: str) -> str:
    weight = WEIGHT_FOR_CHAR.get(char, DEFAULT_WEIGHT)
    font_path = os.path.join(FONTS_DIR, f"Geist-{weight}.ttf")
    font = TTFont(font_path)
    cmap = font.getBestCmap()
    glyph_name = cmap[ord(char)]
    glyph_set = font.getGlyphSet()
    glyph = glyph_set[glyph_name]

    bb = font["glyf"][glyph_name]
    gw = bb.xMax - bb.xMin
    gh = bb.yMax - bb.yMin

    # egui の `painter.text(font_size = body_px * GLYPH_RATIO)` と同じスケール
    # 関係になるよう、グリフのスケールは **em (1000 font units) を基準**にする。
    # 過去版はグリフ外接矩形の高さを target_h に正規化していたため、'H' のように
    # キャップ高がフルの em の 71% 程度しかない文字でも viewBox 内で
    # GLYPH_RATIO ぶん占めてしまい、フォント描画より大きく見えていた。
    upem = font["head"].unitsPerEm
    font_size_in_viewbox = VIEWBOX * GLYPH_RATIO  # 1 em をビューボックス座標で何単位とするか
    scale = font_size_in_viewbox / upem
    target_w = gw * scale
    target_h = gh * scale

    # グリフ外接矩形の中心を viewBox 中心 (24, 24) に持っていく。
    # SVG は y 下向きなので scale Y は反転、yMax は font 座標で上端。
    ox = (VIEWBOX - target_w) / 2.0 - bb.xMin * scale
    oy = (VIEWBOX - target_h) / 2.0 + bb.yMax * scale

    pen = SVGPathPen(glyph_set)
    glyph.draw(pen)
    path = pen.getCommands()

    transform = f"translate({ox:.3f} {oy:.3f}) scale({scale:.6f} {-scale:.6f})"
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {VIEWBOX:.0f} {VIEWBOX:.0f}">
  <g transform="{transform}">
    <path d="{path}" fill="currentColor"/>
  </g>
</svg>
'''


def render_png(svg_path: str, png_path: str) -> None:
    rsvg_convert = shutil.which("rsvg-convert")
    if rsvg_convert:
        subprocess.run(
            [rsvg_convert, "-w", str(RASTER_SIZE), "-h", str(RASTER_SIZE), svg_path, "-o", png_path],
            check=True,
        )
        return

    magick = shutil.which("magick")
    if magick:
        subprocess.run(
            [magick, svg_path, "-background", "none", "-resize", f"{RASTER_SIZE}x{RASTER_SIZE}", png_path],
            check=True,
        )
        return

    raise SystemExit("PNG 生成には rsvg-convert または magick が必要です")


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)
    os.makedirs(OUT_DIR, exist_ok=True)
    for char in sys.argv[1:]:
        if len(char) != 1:
            print(f"skip {char!r}: 1 文字だけ指定可", file=sys.stderr)
            continue
        svg = extract(char)
        filename = FILENAME_FOR_CHAR.get(char, char.lower())
        out_path = os.path.join(OUT_DIR, f"{filename}.svg")
        with open(out_path, "w") as f:
            f.write(svg)
        png_path = os.path.join(OUT_DIR, f"{filename}.png")
        render_png(out_path, png_path)
        weight = WEIGHT_FOR_CHAR.get(char, DEFAULT_WEIGHT)
        print(
            f"wrote {os.path.relpath(out_path, REPO_ROOT)} / "
            f"{os.path.relpath(png_path, REPO_ROOT)}  (Geist {weight})"
        )


if __name__ == "__main__":
    main()
