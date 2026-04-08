/// QRコード生成・読み取り
use image::{GrayImage, Luma, RgbaImage, Rgba};
use crate::layout;

/// QRコードを生成し、指定位置にRGBA画像に描画する
pub fn draw_qr_on_image(img: &mut RgbaImage, data: &str) -> Result<(), String> {
    let code = qrcode::QrCode::new(data.as_bytes())
        .map_err(|e| format!("QRコード生成エラー: {e}"))?;

    let qr_image = code.render::<Luma<u8>>()
        .quiet_zone(false)
        .min_dimensions(1, 1)
        .build();

    let qr_w = qr_image.width();
    let qr_h = qr_image.height();

    let target_w = layout::mm_to_px(layout::QR_SIZE).round() as u32;
    let target_h = target_w;
    let start_x = layout::mm_to_px(layout::QR_X).round() as u32;
    let start_y = layout::mm_to_px(layout::QR_Y).round() as u32;

    for dy in 0..target_h {
        for dx in 0..target_w {
            let src_x = (dx as f64 / target_w as f64 * qr_w as f64) as u32;
            let src_y = (dy as f64 / target_h as f64 * qr_h as f64) as u32;
            let src_x = src_x.min(qr_w - 1);
            let src_y = src_y.min(qr_h - 1);

            let pixel = qr_image.get_pixel(src_x, src_y);
            let px = start_x + dx;
            let py = start_y + dy;
            if px < img.width() && py < img.height() {
                let c = pixel[0];
                img.put_pixel(px, py, Rgba([c, c, c, 255]));
            }
        }
    }

    Ok(())
}

/// グレースケール画像の指定領域からQRコードを読み取る
pub fn read_qr_from_gray(gray: &GrayImage) -> Result<String, String> {
    let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(
        gray.width() as usize,
        gray.height() as usize,
        |x, y| gray.get_pixel(x as u32, y as u32)[0],
    );

    let grids = prepared.detect_grids();
    if grids.is_empty() {
        return Err("QRコードが検出されませんでした".to_string());
    }

    let (_, content) = grids[0]
        .decode()
        .map_err(|e| format!("QRデコードエラー: {e}"))?;

    Ok(content)
}
