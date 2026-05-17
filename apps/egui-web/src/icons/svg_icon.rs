//! SVG 共有アセットから焼き付けた PNG を、単色テクスチャとして描画する。
//!
//! `assets/icons/{h,x,y,z,plus,sqrtx,s,sdagger}.svg` を正にし、
//! `scripts/extract-gate-svg.py` が同じ場所へ 128×128 px の PNG を生成する。
//! ビルド時に PNG のアルファだけを RLE 化して
//! wasm に埋め込み、初回描画時に egui テクスチャへ展開する。実行時の SVG パーサや PNG
//! デコーダは不要。

use eframe::egui;
use std::cell::RefCell;
use std::collections::HashMap;

include!(concat!(env!("OUT_DIR"), "/gate_icon_alpha.rs"));

/// Rust から SVG 由来テクスチャで描画する対象のグリフ。
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(super) enum GateGlyph {
    H,
    /// 文字 X 用。Qni の X ゲート本体は Plus を描くため本番回路では未使用だが、
    /// 共有アセットとしてビルド時にラスタ化を検証する。
    #[expect(dead_code, reason = "X SVG は共有デザインシステム用アセット")]
    X,
    Y,
    Z,
    Plus,
    SqrtX,
    S,
    SDagger,
}

thread_local! {
    static TEXTURE_CACHE: RefCell<HashMap<GateGlyph, egui::TextureHandle>> = RefCell::new(HashMap::new());
}

fn alpha_rle(glyph: GateGlyph) -> &'static [(u16, u8)] {
    match glyph {
        GateGlyph::H => H_ALPHA_RLE,
        GateGlyph::X => X_ALPHA_RLE,
        GateGlyph::Y => Y_ALPHA_RLE,
        GateGlyph::Z => Z_ALPHA_RLE,
        GateGlyph::Plus => PLUS_ALPHA_RLE,
        GateGlyph::SqrtX => SQRTX_ALPHA_RLE,
        GateGlyph::S => S_ALPHA_RLE,
        GateGlyph::SDagger => SDAGGER_ALPHA_RLE,
    }
}

fn texture_name(glyph: GateGlyph) -> &'static str {
    match glyph {
        GateGlyph::H => "gate-icon-h-png-raster",
        GateGlyph::X => "gate-icon-x-png-raster",
        GateGlyph::Y => "gate-icon-y-png-raster",
        GateGlyph::Z => "gate-icon-z-png-raster",
        GateGlyph::Plus => "gate-icon-plus-png-raster",
        GateGlyph::SqrtX => "gate-icon-sqrtx-png-raster",
        GateGlyph::S => "gate-icon-s-png-raster",
        GateGlyph::SDagger => "gate-icon-sdagger-png-raster",
    }
}

fn texture_id(ctx: &egui::Context, glyph: GateGlyph) -> egui::TextureId {
    TEXTURE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache
            .entry(glyph)
            .or_insert_with(|| {
                ctx.load_texture(
                    texture_name(glyph),
                    color_image_from_alpha(alpha_rle(glyph)),
                    egui::TextureOptions::LINEAR,
                )
            })
            .id()
    })
}

fn color_image_from_alpha(alpha_rle: &[(u16, u8)]) -> egui::ColorImage {
    let mut pixels = Vec::with_capacity(RASTER_SIZE * RASTER_SIZE);
    for &(run, alpha) in alpha_rle {
        pixels.extend(std::iter::repeat_n(
            egui::Color32::from_white_alpha(alpha),
            run as usize,
        ));
    }
    debug_assert_eq!(pixels.len(), RASTER_SIZE * RASTER_SIZE);
    egui::ColorImage::new([RASTER_SIZE, RASTER_SIZE], pixels)
}

/// 指定したゲートグリフを `assets/icons/*.svg` 由来のテクスチャで描画する。
pub(super) fn draw_glyph(
    painter: &egui::Painter,
    rect: egui::Rect,
    color: egui::Color32,
    glyph: GateGlyph,
) {
    let uv = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
    painter.add(egui::Shape::image(
        texture_id(painter.ctx(), glyph),
        rect,
        uv,
        color,
    ));
}
