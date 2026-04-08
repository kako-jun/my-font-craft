#[allow(dead_code)]
mod layout;
mod template;
mod pipeline;
mod marker;
mod perspective;
mod qr;
mod cell;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mfc-cli")]
#[command(about = "My Font Craft - 画像処理パイプライン（台形補正デバッグ用）")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// テスト画像（テンプレートPNG）を生成する
    Generate {
        /// 出力パス（デフォルト: debug_output/template.png）
        #[arg(short, long, default_value = "debug_output/template.png")]
        output: PathBuf,
    },
    /// 画像を読み込み、パイプラインを実行する
    Process {
        /// 入力画像パス
        image_path: PathBuf,
        /// 出力ディレクトリ（デフォルト: debug_output）
        #[arg(short, long, default_value = "debug_output")]
        output_dir: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Generate { output } => {
            template::generate_template(&output)
        }
        Commands::Process { image_path, output_dir } => {
            pipeline::run_pipeline(&image_path, &output_dir)
        }
    };

    if let Err(e) = result {
        eprintln!("エラー: {}", e);
        std::process::exit(1);
    }
}
