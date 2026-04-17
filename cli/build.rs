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

    // 環境変数を空文字列フィルタ付きで取得するヘルパ
    // CI では未設定変数が空文字列としてエクスポートされることがあるため
    let env_nonempty = |k: &str| std::env::var(k).ok().filter(|s| !s.is_empty());

    // 優先順位: 明示的オーバーライド → CF Pages → GitHub Actions → git rev-parse → "unknown"
    let sha = env_nonempty("MFC_BUILD_GIT_SHA_OVERRIDE")
        .or_else(|| env_nonempty("CF_PAGES_COMMIT_SHA"))
        .or_else(|| env_nonempty("GITHUB_SHA"))
        .map(|s| s.chars().take(7).collect::<String>())
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "--short=7", "HEAD"])
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                        if s.is_empty() {
                            None
                        } else {
                            Some(s)
                        }
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
