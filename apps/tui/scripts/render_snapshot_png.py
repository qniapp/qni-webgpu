#!/usr/bin/env python3
import argparse
import os
import re
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


def pick_font() -> Path:
    candidates = [
        "/usr/share/fonts/TTF/CaskaydiaMonoNerdFont-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
        "/usr/share/fonts/liberation/LiberationMono-Regular.ttf",
        "/usr/share/fonts/noto/NotoSansMono-Regular.ttf",
        "/usr/share/fonts/noto-cjk/NotoSansMonoCJK-Regular.ttc",
    ]
    for path in candidates:
        if os.path.exists(path):
            return Path(path)
    raise FileNotFoundError("No usable monospace font found in known locations")


def parse_snapshot(path: Path) -> list[str]:
    text = path.read_text(encoding="utf-8")
    lines = []
    separator_count = 0
    for raw in text.splitlines():
        if raw.strip() == "---":
            separator_count += 1
            continue
        if separator_count < 2:
            continue
        if not raw.startswith('"'):
            continue
        match = re.match(r'^"(.*)"', raw)
        if not match:
            continue
        lines.append(match.group(1))
    if not lines:
        raise ValueError("Snapshot body not found or empty")
    return lines


def parse_dump(path: Path) -> tuple[int, int, list[list[tuple[str, tuple[int, int, int] | None, tuple[int, int, int] | None]]]]:
    width = height = None
    cells: list[list[tuple[str, tuple[int, int, int] | None, tuple[int, int, int] | None]]] = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        if raw.startswith("SIZE\t"):
            _, w, h = raw.split("\t", 2)
            width = int(w)
            height = int(h)
            cells = [[(" ", None, None) for _ in range(width)] for _ in range(height)]
            continue
        if raw.startswith("CELL\t"):
            parts = raw.split("\t")
            if len(parts) < 6:
                continue
            _, xs, ys, sym, fg_s, bg_s = parts[:6]
            if width is None or height is None:
                continue
            x = int(xs)
            y = int(ys)
            fg = None if fg_s == "-" else tuple(int(fg_s[i : i + 2], 16) for i in (1, 3, 5))
            bg = None if bg_s == "-" else tuple(int(bg_s[i : i + 2], 16) for i in (1, 3, 5))
            sym = sym or " "
            if 0 <= y < height and 0 <= x < width:
                cells[y][x] = (sym, fg, bg)
    if width is None or height is None:
        raise ValueError("SIZE header not found in dump")
    return width, height, cells


def cell_metrics(font: ImageFont.FreeTypeFont, cell_w: int | None, cell_h: int | None) -> tuple[int, int]:
    if cell_w is None:
        bbox = font.getbbox("M")
        cell_w = max(1, bbox[2] - bbox[0])
    if cell_h is None:
        ascent, descent = font.getmetrics()
        cell_h = max(1, ascent + descent)
    return cell_w, cell_h


def render_lines(
    lines: list[str],
    font_path: Path,
    font_size: int,
    out_path: Path,
    cell_w: int | None,
    cell_h: int | None,
) -> None:
    font = ImageFont.truetype(str(font_path), font_size)
    cell_w, cell_h = cell_metrics(font, cell_w, cell_h)

    width = max(len(line) for line in lines) * cell_w
    height = len(lines) * cell_h
    image = Image.new("RGB", (width, height), (0, 0, 0))
    draw = ImageDraw.Draw(image)

    for row, line in enumerate(lines):
        draw.text((0, row * cell_h), line, font=font, fill=(255, 255, 255))

    out_path.parent.mkdir(parents=True, exist_ok=True)
    image.save(out_path)


def render_cells(
    width: int,
    height: int,
    cells: list[list[tuple[str, tuple[int, int, int] | None, tuple[int, int, int] | None]]],
    font_path: Path,
    font_size: int,
    out_path: Path,
    cell_w: int | None,
    cell_h: int | None,
) -> None:
    font = ImageFont.truetype(str(font_path), font_size)
    cell_w, cell_h = cell_metrics(font, cell_w, cell_h)

    image = Image.new("RGB", (width * cell_w, height * cell_h), (0, 0, 0))
    draw = ImageDraw.Draw(image)

    for y in range(height):
        for x in range(width):
            sym, fg, bg = cells[y][x]
            if bg is not None:
                draw.rectangle(
                    [x * cell_w, y * cell_h, (x + 1) * cell_w, (y + 1) * cell_h],
                    fill=bg,
                )
            if sym.strip():
                draw.text(
                    (x * cell_w, y * cell_h),
                    sym,
                    font=font,
                    fill=fg or (255, 255, 255),
                )

    out_path.parent.mkdir(parents=True, exist_ok=True)
    image.save(out_path)


def main() -> None:
    parser = argparse.ArgumentParser(description="Render insta snapshot text to PNG.")
    parser.add_argument("snapshot", type=Path, help="Path to .snap or .dump file")
    parser.add_argument("output", type=Path, help="Output PNG path")
    parser.add_argument("--font", type=Path, default=None, help="Optional TTF/TTC font path")
    parser.add_argument("--size", type=int, default=16, help="Font size in pixels")
    parser.add_argument("--cell-width", type=int, default=None, help="Override cell width in pixels")
    parser.add_argument("--cell-height", type=int, default=None, help="Override cell height in pixels")
    args = parser.parse_args()

    font_path = args.font or pick_font()
    if args.snapshot.suffix == ".dump":
        width, height, cells = parse_dump(args.snapshot)
        render_cells(
            width,
            height,
            cells,
            font_path,
            args.size,
            args.output,
            args.cell_width,
            args.cell_height,
        )
    else:
        lines = parse_snapshot(args.snapshot)
        render_lines(
            lines,
            font_path,
            args.size,
            args.output,
            args.cell_width,
            args.cell_height,
        )


if __name__ == "__main__":
    main()
