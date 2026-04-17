use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // HEAD 自体とコミットログの更新で再実行（packed-refs 環境でも .git/logs/HEAD は更新される）
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/logs/HEAD");
    // CF Pages 等のビルド環境変数を監視
    println!("cargo:rerun-if-env-changed=CF_PAGES_COMMIT_SHA");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    println!("cargo:rerun-if-env-changed=MFC_BUILD_GIT_SHA_OVERRIDE");

    // 優先順位: 明示的オーバーライド → CF Pages → GitHub Actions → git rev-parse → "unknown"
    let sha = std::env::var("MFC_BUILD_GIT_SHA_OVERRIDE")
        .ok()
        .or_else(|| std::env::var("CF_PAGES_COMMIT_SHA").ok())
        .or_else(|| std::env::var("GITHUB_SHA").ok())
        .map(|s| s.chars().take(7).collect::<String>())
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                    } else {
                        None
                    }
                })
        })
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=MFC_BUILD_GIT_SHA={sha}");

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    println!("cargo:rustc-env=MFC_BUILD_UNIX_TS={ts}");
}
