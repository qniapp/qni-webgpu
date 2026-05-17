#!/usr/bin/env python3
"""Geist フォントからゲートのグリフアウトラインを抽出し、
`apps/egui-web/assets/icons/<name>.svg` と同名 PNG に書き出す。

design-system-architecture.html の Tier A 提案 (SVG アイコンを Rust と
TypeScript で 1 ソース共有) に従い、ゲートアイコンの文字部分を
フォント由来の SVG ファイルとして固定化するための補助スクリプト。

使い方:
    python3 scripts/extract-gate-svg.py H
    python3 scripts/extract-gate-svg.py H X Y Z + √X S S† T T† P RX RY RZ QFT QFT†
    python3 scripts/extract-gate-svg.py 0 1

設定:
    - 単一字ゲートは Geist Regular 400
    - "+" (X ゲートの本体) は Geist Medium 500
    - Write / Measurement 用の 0 / 1 は Geist Mono Regular
    - √X は Geist Regular の √ と X を合成
    - S† / T† は Geist Regular の基底文字と † を合成
    - RX / RY / RZ は Geist Medium 500 の 2 文字を 0.46 倍で合成
    - QFT / QFT† は Geist Medium 500 の 3 文字を 0.46 倍で合成
    - viewBox は 48×48 (apps/egui-web/src/icons.rs の VIEWBOX)
    - 単一字グリフ高さは viewBox の 0.62 倍

依存:
    pip install --user --break-system-packages fonttools
    sudo apt install librsvg2-bin  # rsvg-convert。無い場合は ImageMagick の magick を使う。
"""

import os
import shutil
import subprocess
import sys
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.ttLib import TTFont

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
FONTS_DIR = os.path.join(REPO_ROOT, "apps/egui-web/assets")
OUT_DIR = os.path.join(REPO_ROOT, "apps/egui-web/assets/icons")
VIEWBOX = 48.0
RASTER_SIZE = 256
GLYPH_RATIO = 0.62
# 2 文字の回転ラベルは 0.40 倍だと H / S / P より線が細く見えるため、
# 32 px ゲート内に収まる範囲で少し大きくする。
MULTI_GLYPH_RATIO = 0.46
SQRTX_RADICAL_RATIO = 0.85
SQRTX_RADICAL_LIFT = 0.05
DAGGER_RATIO = 0.32
DAGGER_INSET_X = 0.17
DAGGER_INSET_Y = 0.22

WEIGHT_FOR_CHAR = {
    "+": "Medium",
    "0": "MonoRegular",
    "1": "MonoRegular",
}
RATIO_FOR_CHAR = {
    # SDF 化した Measurement digit と同じ 13 px 高に合わせる。
    "0": 0.58,
    "1": 0.58,
}
WEIGHT_FOR_TOKEN = {
    "RX": "Medium",
    "RY": "Medium",
    "RZ": "Medium",
    "QFT": "Medium",
}
RATIO_FOR_TOKEN = {
    "RX": MULTI_GLYPH_RATIO,
    "RY": MULTI_GLYPH_RATIO,
    "RZ": MULTI_GLYPH_RATIO,
    "QFT": 0.46,
}
DEFAULT_WEIGHT = "Regular"

FILENAME_FOR_TOKEN = {
    "+": "plus",
    "0": "digit0",
    "1": "digit1",
    "√X": "sqrtx",
    "S†": "sdagger",
    "T†": "tdagger",
    "QFT†": "qftdagger",
}

SQRTX_ALIASES = {"√X", "sqrtx", "SqrtX", "SQRTX", "X^½"}
SDAGGER_ALIASES = {"S†", "sdagger", "SDagger", "SDAGGER"}
TDAGGER_ALIASES = {"T†", "tdagger", "TDagger", "TDAGGER"}
QFTDAGGER_ALIASES = {"QFT†", "qftdagger", "QftDagger", "QFTDAGGER"}
ROTATION_ALIASES = {
    "RX": "RX",
    "Rx": "RX",
    "rx": "RX",
    "RY": "RY",
    "Ry": "RY",
    "ry": "RY",
    "RZ": "RZ",
    "Rz": "RZ",
    "rz": "RZ",
}


def normalize_token(token: str) -> str:
    if token in SQRTX_ALIASES:
        return "√X"
    if token in SDAGGER_ALIASES:
        return "S†"
    if token in TDAGGER_ALIASES:
        return "T†"
    if token in QFTDAGGER_ALIASES:
        return "QFT†"
    if token in ROTATION_ALIASES:
        return ROTATION_ALIASES[token]
    return token


def font_for_weight(weight: str) -> TTFont:
    if weight == "MonoRegular":
        return TTFont(os.path.join(FONTS_DIR, "GeistMono-Regular.ttf"))
    return TTFont(os.path.join(FONTS_DIR, f"Geist-{weight}.ttf"))


def glyph_path_and_box(char: str, weight: str):
    font = font_for_weight(weight)
    cmap = font.getBestCmap()
    glyph_name = cmap[ord(char)]
    glyph_set = font.getGlyphSet()
    glyph = glyph_set[glyph_name]
    box = font["glyf"][glyph_name]
    pen = SVGPathPen(glyph_set)
    glyph.draw(pen)
    return font, box, pen.getCommands()


def glyph_advance(char: str, weight: str) -> int:
    font = font_for_weight(weight)
    glyph_name = font.getBestCmap()[ord(char)]
    advance, _ = font["hmtx"].metrics[glyph_name]
    return advance


def transform_for_box(box, scale: float, left: float, top: float) -> str:
    ox = left - box.xMin * scale
    oy = top + box.yMax * scale
    return f"translate({ox:.3f} {oy:.3f}) scale({scale:.6f} {-scale:.6f})"


def path_element(char: str, weight: str, scale: float, left: float, top: float) -> str:
    _, box, path = glyph_path_and_box(char, weight)
    transform = transform_for_box(box, scale, left, top)
    return f'''  <g transform="{transform}">
    <path d="{path}" fill="currentColor"/>
  </g>'''


def glyph_box_size(char: str, weight: str, scale: float):
    _, box, _ = glyph_path_and_box(char, weight)
    return (box.xMax - box.xMin) * scale, (box.yMax - box.yMin) * scale


def single_glyph_svg(char: str) -> str:
    weight = WEIGHT_FOR_CHAR.get(char, DEFAULT_WEIGHT)
    font = font_for_weight(weight)
    _, box, _ = glyph_path_and_box(char, weight)

    # グリフのスケールは **em (1000 font units) を基準**にし、
    # SDF 化しても 32px ゲート上の文字サイズが揃うようにする。
    # 過去版はグリフ外接矩形の高さを target_h に正規化していたため、'H' のように
    # キャップ高がフルの em の 71% 程度しかない文字でも viewBox 内で
    # GLYPH_RATIO ぶん占めてしまい、フォント描画より大きく見えていた。
    scale = VIEWBOX * RATIO_FOR_CHAR.get(char, GLYPH_RATIO) / font["head"].unitsPerEm
    target_w = (box.xMax - box.xMin) * scale
    target_h = (box.yMax - box.yMin) * scale

    # グリフ外接矩形の中心を viewBox 中心 (24, 24) に持っていく。
    left = (VIEWBOX - target_w) / 2.0
    top = (VIEWBOX - target_h) / 2.0
    body = path_element(char, weight, scale, left, top)
    return svg_document(body)


def sqrtx_svg() -> str:
    weight = DEFAULT_WEIGHT
    font = font_for_weight(weight)
    x_scale = VIEWBOX * GLYPH_RATIO / font["head"].unitsPerEm
    radical_scale = x_scale * SQRTX_RADICAL_RATIO
    radical_w, radical_h = glyph_box_size("√", weight, radical_scale)
    x_w, x_h = glyph_box_size("X", weight, x_scale)

    total_w = radical_w + x_w
    left = VIEWBOX / 2.0 - total_w / 2.0
    x_left = left + radical_w
    x_top = VIEWBOX / 2.0 - x_h / 2.0
    radical_left = left
    radical_bottom = x_top + x_h - VIEWBOX * SQRTX_RADICAL_LIFT
    radical_top = radical_bottom - radical_h

    body = "\n".join(
        [
            path_element("√", weight, radical_scale, radical_left, radical_top),
            path_element("X", weight, x_scale, x_left, x_top),
        ]
    )
    return svg_document(body)


def multi_glyph_body(text: str, weight: str, scale: float) -> str:
    glyphs = []
    cursor = 0
    min_x = float("inf")
    min_y = float("inf")
    max_x = float("-inf")
    max_y = float("-inf")
    for char in text:
        _, box, _ = glyph_path_and_box(char, weight)
        glyphs.append((char, box, cursor))
        min_x = min(min_x, cursor + box.xMin)
        max_x = max(max_x, cursor + box.xMax)
        min_y = min(min_y, box.yMin)
        max_y = max(max_y, box.yMax)
        cursor += glyph_advance(char, weight)

    union_w = (max_x - min_x) * scale
    union_h = (max_y - min_y) * scale
    origin_x = (VIEWBOX - union_w) / 2.0 - min_x * scale
    baseline_y = (VIEWBOX - union_h) / 2.0 + max_y * scale
    return "\n".join(
        path_element(
            char,
            weight,
            scale,
            origin_x + (cursor + box.xMin) * scale,
            baseline_y - box.yMax * scale,
        )
        for char, box, cursor in glyphs
    )


def multi_glyph_svg(text: str) -> str:
    weight = WEIGHT_FOR_TOKEN[text]
    font = font_for_weight(weight)
    scale = VIEWBOX * RATIO_FOR_TOKEN[text] / font["head"].unitsPerEm
    return svg_document(multi_glyph_body(text, weight, scale))


def qft_dagger_svg() -> str:
    weight = WEIGHT_FOR_TOKEN["QFT"]
    font = font_for_weight(weight)
    base_scale = VIEWBOX * RATIO_FOR_TOKEN["QFT"] / font["head"].unitsPerEm
    dagger_scale = VIEWBOX * DAGGER_RATIO / font["head"].unitsPerEm
    dagger_w, dagger_h = glyph_box_size("†", weight, dagger_scale)
    dagger_cx = VIEWBOX - VIEWBOX * DAGGER_INSET_X
    dagger_cy = VIEWBOX * DAGGER_INSET_Y
    dagger_left = dagger_cx - dagger_w / 2.0
    dagger_top = dagger_cy - dagger_h / 2.0
    body = "\n".join(
        [
            multi_glyph_body("QFT", weight, base_scale),
            path_element("†", weight, dagger_scale, dagger_left, dagger_top),
        ]
    )
    return svg_document(body)


def dagger_svg(base_char: str) -> str:
    weight = DEFAULT_WEIGHT
    font = font_for_weight(weight)
    base_scale = VIEWBOX * GLYPH_RATIO / font["head"].unitsPerEm
    dagger_scale = VIEWBOX * DAGGER_RATIO / font["head"].unitsPerEm
    base_w, base_h = glyph_box_size(base_char, weight, base_scale)
    dagger_w, dagger_h = glyph_box_size("†", weight, dagger_scale)

    base_left = (VIEWBOX - base_w) / 2.0
    base_top = (VIEWBOX - base_h) / 2.0
    dagger_cx = VIEWBOX - VIEWBOX * DAGGER_INSET_X
    dagger_cy = VIEWBOX * DAGGER_INSET_Y
    dagger_left = dagger_cx - dagger_w / 2.0
    dagger_top = dagger_cy - dagger_h / 2.0

    body = "\n".join(
        [
            path_element(base_char, weight, base_scale, base_left, base_top),
            path_element("†", weight, dagger_scale, dagger_left, dagger_top),
        ]
    )
    return svg_document(body)


def svg_document(body: str) -> str:
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {VIEWBOX:.0f} {VIEWBOX:.0f}">
{body}
</svg>
'''


def extract(token: str) -> str:
    if token == "√X":
        return sqrtx_svg()
    if token == "S†":
        return dagger_svg("S")
    if token == "T†":
        return dagger_svg("T")
    if token == "QFT†":
        return qft_dagger_svg()
    if token in WEIGHT_FOR_TOKEN:
        return multi_glyph_svg(token)
    if len(token) == 1:
        return single_glyph_svg(token)
    raise ValueError(f"未対応トークン: {token!r}")


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


def weight_description(token: str) -> str:
    if token in {"√X", "S†", "T†"}:
        return "Geist Regular composite"
    if token in WEIGHT_FOR_TOKEN or token == "QFT†":
        return "Geist Medium composite"
    if WEIGHT_FOR_CHAR.get(token) == "MonoRegular":
        return "Geist Mono Regular"
    return f"Geist {WEIGHT_FOR_CHAR.get(token, DEFAULT_WEIGHT)}"


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)
    os.makedirs(OUT_DIR, exist_ok=True)
    for raw_token in sys.argv[1:]:
        token = normalize_token(raw_token)
        svg = extract(token)
        filename = FILENAME_FOR_TOKEN.get(token, token.lower())
        out_path = os.path.join(OUT_DIR, f"{filename}.svg")
        with open(out_path, "w") as f:
            f.write(svg)
        png_path = os.path.join(OUT_DIR, f"{filename}.png")
        render_png(out_path, png_path)
        print(
            f"wrote {os.path.relpath(out_path, REPO_ROOT)} / "
            f"{os.path.relpath(png_path, REPO_ROOT)}  ({weight_description(token)})"
        )


if __name__ == "__main__":
    main()
