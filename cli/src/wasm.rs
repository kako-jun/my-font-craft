/// WASM エントリポイント（#[wasm_bindgen] 付き公開関数）
use wasm_bindgen::prelude::*;
use crate::pipeline;

/// WASM初期化（パニックフック設定）
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// 画像バイト列を受け取り、処理結果をJSONで返す
///
/// 入力: JPEG/PNGの生バイト列
/// 出力: ProcessResult をJSON化した JsValue
#[wasm_bindgen]
pub fn process_image(image_bytes: &[u8]) -> Result<JsValue, JsValue> {
    let result = pipeline::process_image_bytes(image_bytes)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&result)
        .map_err(|e| JsValue::from_str(&format!("シリアライズエラー: {e}")))
}
