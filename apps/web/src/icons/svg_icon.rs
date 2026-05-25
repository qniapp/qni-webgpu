//! SVG 共有アセットから焼き付けた PNG を、単色アイコンとして描画する。
//!
//! `assets/icons/{h,x,y,z,plus,sqrtx,s,sdagger,t,tdagger,p,rx,ry,rz,qft,qftdagger,digit0,digit1}.svg` を正にし、
//! `p.svg` は Phase ゲートの Φ ラベルを保持する。
//! `scripts/extract-gate-svg.py` が同じ場所へ 256×256 px の PNG を生成する。
//! WebGPU 経路では PNG アルファから生成した SDF（符号付き距離場）を
//! `sdf_icon` のシェーダで描く。WebGPU が無い経路だけ、同じ PNG アルファの
//! RLE から作った通常テクスチャへ戻す。

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
    X,
    Y,
    Z,
    Plus,
    SqrtX,
    S,
    SDagger,
    T,
    TDagger,
    Phase,
    Rx,
    Ry,
    Rz,
    Qft,
    QftDagger,
    Digit0,
    Digit1,
}

pub(crate) const DIGIT_SDF_SIZE: usize = RASTER_SIZE;
pub(crate) const DIGIT_SDF_PX_RANGE: f32 = SDF_PX_RANGE;

impl GateGlyph {
    pub(super) const ALL: [Self; 18] = [
        Self::H,
        Self::X,
        Self::Y,
        Self::Z,
        Self::Plus,
        Self::SqrtX,
        Self::S,
        Self::SDagger,
        Self::T,
        Self::TDagger,
        Self::Phase,
        Self::Rx,
        Self::Ry,
        Self::Rz,
        Self::Qft,
        Self::QftDagger,
        Self::Digit0,
        Self::Digit1,
    ];
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
        GateGlyph::T => T_ALPHA_RLE,
        GateGlyph::TDagger => TDAGGER_ALPHA_RLE,
        GateGlyph::Phase => P_ALPHA_RLE,
        GateGlyph::Rx => RX_ALPHA_RLE,
        GateGlyph::Ry => RY_ALPHA_RLE,
        GateGlyph::Rz => RZ_ALPHA_RLE,
        GateGlyph::Qft => QFT_ALPHA_RLE,
        GateGlyph::QftDagger => QFTDAGGER_ALPHA_RLE,
        GateGlyph::Digit0 => DIGIT0_ALPHA_RLE,
        GateGlyph::Digit1 => DIGIT1_ALPHA_RLE,
    }
}

pub(super) fn sdf_rle(glyph: GateGlyph) -> &'static [(u16, u8)] {
    match glyph {
        GateGlyph::H => H_SDF_RLE,
        GateGlyph::X => X_SDF_RLE,
        GateGlyph::Y => Y_SDF_RLE,
        GateGlyph::Z => Z_SDF_RLE,
        GateGlyph::Plus => PLUS_SDF_RLE,
        GateGlyph::SqrtX => SQRTX_SDF_RLE,
        GateGlyph::S => S_SDF_RLE,
        GateGlyph::SDagger => SDAGGER_SDF_RLE,
        GateGlyph::T => T_SDF_RLE,
        GateGlyph::TDagger => TDAGGER_SDF_RLE,
        GateGlyph::Phase => P_SDF_RLE,
        GateGlyph::Rx => RX_SDF_RLE,
        GateGlyph::Ry => RY_SDF_RLE,
        GateGlyph::Rz => RZ_SDF_RLE,
        GateGlyph::Qft => QFT_SDF_RLE,
        GateGlyph::QftDagger => QFTDAGGER_SDF_RLE,
        GateGlyph::Digit0 => DIGIT0_SDF_RLE,
        GateGlyph::Digit1 => DIGIT1_SDF_RLE,
    }
}

pub(crate) fn digit_sdf_rle(digit: u32) -> &'static [(u16, u8)] {
    match digit {
        0 => DIGIT0_SDF_RLE,
        1 => DIGIT1_SDF_RLE,
        _ => panic!("digit SDF only supports 0 and 1"),
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
        GateGlyph::T => "gate-icon-t-png-raster",
        GateGlyph::TDagger => "gate-icon-tdagger-png-raster",
        GateGlyph::Phase => "gate-icon-phase-png-raster",
        GateGlyph::Rx => "gate-icon-rx-png-raster",
        GateGlyph::Ry => "gate-icon-ry-png-raster",
        GateGlyph::Rz => "gate-icon-rz-png-raster",
        GateGlyph::Qft => "gate-icon-qft-png-raster",
        GateGlyph::QftDagger => "gate-icon-qftdagger-png-raster",
        GateGlyph::Digit0 => "gate-icon-digit0-png-raster",
        GateGlyph::Digit1 => "gate-icon-digit1-png-raster",
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
    if super::sdf_icon::draw_glyph(painter, rect, color, glyph) {
        return;
    }

    let uv = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
    painter.add(egui::Shape::image(
        texture_id(painter.ctx(), glyph),
        rect,
        uv,
        color,
    ));
}
