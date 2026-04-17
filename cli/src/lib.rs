/// WASM/CLI共用ログマクロ
/// WASM: web_sys::console::log_1 に出力
/// CLI: println! に出力
#[cfg(target_arch = "wasm32")]
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        web_sys::console::log_1(&format!($($arg)*).into())
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        println!($($arg)*)
    }
}

pub mod layout;
pub mod marker;
pub mod perspective;
pub mod qr;
pub mod cell;
pub mod vectorizer;
pub mod pipeline;
pub mod template;
pub mod distort;

#[cfg(target_arch = "wasm32")]
mod wasm;
