/// ページ四隅外挿 + 射影変換（双線形マッピング）
use image::{RgbaImage, Rgba};
use crate::layout;
use crate::marker::DetectedMarker;

/// ページ四隅の座標
#[derive(Debug, Clone)]
pub struct PageCorners {
    pub tl: (f64, f64),
    pub tr: (f64, f64),
    pub bl: (f64, f64),
    pub br: (f64, f64),
}

/// マーカー中心座標からページ四隅を線形外挿する
/// TypeScript extrapolatePageCorners の移植
pub fn extrapolate_page_corners(markers: &[DetectedMarker; 4]) -> PageCorners {
    let ml = &markers[0]; // TL
    let mr = &markers[1]; // TR
    let bl = &markers[2]; // BL
    let br = &markers[3]; // BR

    // マーカー中心のページ内座標（mm）
    let m_left = layout::MARKER_TL.x + layout::MARKER_SIZE / 2.0;   // 3 + 4 = 7
    let m_right = layout::MARKER_TR.x + layout::MARKER_SIZE / 2.0;  // 201 + 4 = 205
    let m_top = layout::MARKER_TL.y + layout::MARKER_SIZE / 2.0;    // 3 + 4 = 7
    let m_bottom = layout::MARKER_BL.y + layout::MARKER_SIZE / 2.0; // 289 + 4 = 293

    let ratio_left = m_left / (m_right - m_left);             // 7 / 198
    let ratio_right = (layout::PAGE_WIDTH - m_right) / (m_right - m_left); // 5 / 198
    let ratio_top = m_top / (m_bottom - m_top);               // 7 / 286
    let ratio_bottom = (layout::PAGE_HEIGHT - m_bottom) / (m_bottom - m_top); // 4 / 286

    println!("  外挿比率: left={ratio_left:.4} right={ratio_right:.4} top={ratio_top:.4} bottom={ratio_bottom:.4}");

    // TL隅
    let tl_x = ml.cx - (mr.cx - ml.cx) * ratio_left;
    let tl_y = ml.cy - (bl.cy - ml.cy) * ratio_top;

    // TR隅
    let tr_x = mr.cx + (mr.cx - ml.cx) * ratio_right;
    let tr_y = mr.cy - (br.cy - mr.cy) * ratio_top;

    // BL隅
    let bl_x = bl.cx - (br.cx - bl.cx) * ratio_left;
    let bl_y = bl.cy + (bl.cy - ml.cy) * ratio_bottom;

    // BR隅
    let br_x = br.cx + (br.cx - bl.cx) * ratio_right;
    let br_y = br.cy + (br.cy - mr.cy) * ratio_bottom;

    let corners = PageCorners {
        tl: (tl_x, tl_y),
        tr: (tr_x, tr_y),
        bl: (bl_x, bl_y),
        br: (br_x, br_y),
    };

    println!("  外挿ページ四隅:");
    println!("    TL=({:.1}, {:.1})", corners.tl.0, corners.tl.1);
    println!("    TR=({:.1}, {:.1})", corners.tr.0, corners.tr.1);
    println!("    BL=({:.1}, {:.1})", corners.bl.0, corners.bl.1);
    println!("    BR=({:.1}, {:.1})", corners.br.0, corners.br.1);

    corners
}

/// 双線形マッピングで台形→矩形に射影変換
pub fn bilinear_warp(img: &RgbaImage, corners: &PageCorners) -> RgbaImage {
    // ターゲットサイズ: 300dpi相当
    let target_w = layout::image_width();
    let target_h = layout::image_height();

    println!("  射影変換: {}x{} → {}x{}", img.width(), img.height(), target_w, target_h);

    let mut out = RgbaImage::new(target_w, target_h);

    for dy in 0..target_h {
        for dx in 0..target_w {
            let u = dx as f64 / target_w as f64;
            let v = dy as f64 / target_h as f64;

            let src_x = (1.0 - u) * (1.0 - v) * corners.tl.0
                + u * (1.0 - v) * corners.tr.0
                + (1.0 - u) * v * corners.bl.0
                + u * v * corners.br.0;
            let src_y = (1.0 - u) * (1.0 - v) * corners.tl.1
                + u * (1.0 - v) * corners.tr.1
                + (1.0 - u) * v * corners.bl.1
                + u * v * corners.br.1;

            // nearest neighbor sampling
            let sx = src_x.round() as i64;
            let sy = src_y.round() as i64;

            if sx >= 0 && sy >= 0 && (sx as u32) < img.width() && (sy as u32) < img.height() {
                out.put_pixel(dx, dy, *img.get_pixel(sx as u32, sy as u32));
            } else {
                out.put_pixel(dx, dy, Rgba([255, 255, 255, 255]));
            }
        }
    }

    out
}
