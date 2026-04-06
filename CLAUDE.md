# MyFontCraft - 開発ガイド

## プロジェクト概要

手書きテンプレートからオリジナルフォントを作成するWebサービス。
全処理がブラウザ内で完結。

## ドキュメント

| ファイル                                               | 内容                       |
| ------------------------------------------------------ | -------------------------- |
| [README.md](./README.md)                               | エンドユーザー向け説明     |
| [docs/README.md](./docs/README.md)                     | ドキュメント目次           |
| [docs/overview.md](./docs/overview.md)                 | プロジェクト概要・設計方針 |
| [docs/characters.md](./docs/characters.md)             | 対象文字リスト             |
| [docs/template-spec.md](./docs/template-spec.md)       | テンプレート仕様           |
| [docs/image-processing.md](./docs/image-processing.md) | 画像処理仕様               |
| [docs/font-generation.md](./docs/font-generation.md)   | フォント生成仕様           |
| [docs/ui-design.md](./docs/ui-design.md)               | UI/UXデザイン              |
| [docs/tech-stack.md](./docs/tech-stack.md)             | 技術スタック               |
| [docs/future-plans.md](./docs/future-plans.md)         | 将来の拡張・改良ネタ       |

## 技術スタック

- **UIフレームワーク**: Solid.js
- **ビルドツール**: Vite + TypeScript
- **ホスティング**: Cloudflare Pages
- **PDF生成**: pdf-lib
- **画像処理**: Canvas API
- **フォント生成**: opentype.js
- **QRコード**: jsQR, qrcode
- **ZIP展開**: JSZip

## ディレクトリ構成（予定）

```
my-font-craft/
├── docs/
│   ├── README.md
│   ├── overview.md
│   ├── characters.md
│   ├── template-spec.md
│   ├── image-processing.md
│   ├── font-generation.md
│   ├── ui-design.md
│   ├── tech-stack.md
│   └── future-plans.md
├── src/
│   ├── index.tsx           # Solid.jsエントリーポイント
│   ├── App.tsx             # ルートコンポーネント
│   ├── pages/              # ページコンポーネント
│   ├── components/         # 共通コンポーネント
│   ├── lib/
│   │   ├── template/       # テンプレートPDF生成
│   │   ├── scanner/        # 画像処理・文字切り出し
│   │   ├── vectorizer/     # ベクター化
│   │   └── font/           # フォント生成
│   ├── data/
│   │   ├── characters.ts   # 対象文字リスト
│   │   └── joyo-kanji.ts   # 常用漢字リスト
│   └── styles/
│       └── global.css
├── public/                 # 静的ファイル
├── README.md
├── CLAUDE.md
├── package.json
├── tsconfig.json
└── wrangler.toml           # Cloudflare設定
```

## 主要機能モジュール

### 1. テンプレート生成 (`src/lib/template/`)

- PDF生成（pdf-lib）
- QRコード埋め込み
- グレースケールバー描画
- 四隅マーカー配置（ドット絵風）

詳細: [docs/template-spec.md](./docs/template-spec.md)

### 2. 画像処理 (`src/lib/scanner/`)

- QRコード読み取り（jsQR）
- 台形補正（四隅マーカー）
- 色補正（グレースケールバー基準）
- シアン除去
- マス切り出し
- ✓/×/空欄 検出（AI使用）

詳細: [docs/image-processing.md](./docs/image-processing.md)

### 3. ベクター化 (`src/lib/vectorizer/`)

- 輪郭抽出
- ビットマップ → ベクター変換
- ベジェ曲線変換

詳細: [docs/font-generation.md](./docs/font-generation.md)

### 4. フォント生成 (`src/lib/font/`)

- グリフ登録
- calt（Contextual Alternates）設定
- TTF/OTF出力（opentype.js）

詳細: [docs/font-generation.md](./docs/font-generation.md)

## 設計原則

- サーバー処理なし（全部ブラウザ）
- ユーザー登録なし
- データベースなし
- プライバシー重視（画像をサーバーに送らない）
- AIで文字を生成しない（本人の字を100%使用）

## デザインコンセプト

- **スキューモーフィズム**: 紙のイメージ
- **ドット絵風アクセント**: 楽しさ
- **マインクラフト風ロゴ**: MyFontCraft

詳細: [docs/ui-design.md](./docs/ui-design.md)

## 開発コマンド

```bash
# 開発サーバー起動
npm run dev

# ビルド
npm run build

# テスト実行
npm test

# テスト（ウォッチモード）
npm run test:watch

# 模擬スキャン画像の再生成
npm run test:generate-fixtures

# リント（ESLint）
npm run lint

# フォーマット（Prettier）
npm run format

# E2Eテスト（Playwright）
npm run test:e2e

# E2Eテスト（UIモード）
npm run test:e2e:ui

# リント + フォーマットチェック
npm run check

# デプロイ
npm run deploy
```

## 将来の拡張

- 他言語対応（インド、アラビア語、ハングル等）
- スタンプ・コラージュからのフォント作成
- 表示時のバリエーション切り替え

詳細: [docs/future-plans.md](./docs/future-plans.md)
