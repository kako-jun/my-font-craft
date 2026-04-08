# 技術スタック

## アーキテクチャ

```
┌─────────────────────────────────────┐
│          ブラウザ（クライアント）        │
│                                     │
│  ┌──────────┐  ┌──────────┐        │
│  │ UI/UX    │  │ 処理エンジン │        │
│  │(Solid.js)│  │           │        │
│  │          │  │ - PDF生成  │        │
│  │          │  │ - 画像処理  │        │
│  │          │  │ - フォント生成│        │
│  └──────────┘  └──────────┘        │
│                                     │
└─────────────────────────────────────┘
           ↓ 静的ファイル配信のみ
┌─────────────────────────────────────┐
│       Cloudflare Pages              │
│                                     │
│  - HTML/CSS/JS配信                   │
│  - サーバー処理なし                   │
│                                     │
└─────────────────────────────────────┘
```

**重要**: 全処理がブラウザ内で完結。サーバーは静的ファイル配信のみ。

---

## フロントエンド

### フレームワーク

| 技術         | 用途             |
| ------------ | ---------------- |
| **Solid.js** | UIフレームワーク |
| Vite         | ビルドツール     |
| TypeScript   | 型安全な開発     |

### なぜSolid.jsか

- **高パフォーマンス**: 仮想DOMなしの真のリアクティブ性
- **軽量**: バンドルサイズが小さい（React比で約1/3）
- **React風の書き味**: JSXで馴染みやすい
- **細粒度リアクティビティ**: 必要な部分だけ再レンダリング
- **シンプルなメンタルモデル**: コンポーネントは一度だけ実行

### Solid.jsの基本

```tsx
import { createSignal, For, Show } from 'solid-js';

// シグナル（リアクティブな状態）
const [count, setCount] = createSignal(0);

// コンポーネント例
function Counter() {
  const [count, setCount] = createSignal(0);

  return (
    <button onClick={() => setCount(c => c + 1)}>
      Count: {count()}
    </button>
  );
}

// 条件分岐
<Show when={isLoading()} fallback={<Content />}>
  <Loading />
</Show>

// リスト描画
<For each={items()}>
  {(item) => <Item data={item} />}
</For>
```

### 補足: 他フレームワークとの比較

| フレームワーク | 不採用理由                                          |
| -------------- | --------------------------------------------------- |
| React          | バンドルサイズが大きい、仮想DOM のオーバーヘッド    |
| Preact         | Solid.jsの方がパフォーマンス面で優位                |
| Svelte         | 良い選択肢だが、Solid.jsの学習目的で不採用          |
| Astro          | MPAベースでSPAとしての操作感に不向き                |
| Hono           | サーバーサイド/エッジ向け、ブラウザ完結方針と不適合 |

---

## ライブラリ

### PDF生成

| ライブラリ  | 用途                |
| ----------- | ------------------- |
| **pdf-lib** | テンプレートPDF生成 |

```typescript
import { PDFDocument, rgb } from 'pdf-lib';

const pdfDoc = await PDFDocument.create();
const page = pdfDoc.addPage([595.28, 841.89]); // A4サイズ（ポイント）
```

### QRコード

| ライブラリ | 用途                                 |
| ---------- | ------------------------------------ |
| **qrcode** | QRコード生成（テンプレート用）       |
| **jsQR**   | QRコード読み取り（スキャン画像から） |

```typescript
// 生成
import QRCode from 'qrcode';
const qrDataUrl = await QRCode.toDataURL(JSON.stringify(metadata));

// 読み取り
import jsQR from 'jsqr';
const code = jsQR(imageData, width, height);
```

### 画像処理

| ライブラリ     | 用途                           |
| -------------- | ------------------------------ |
| **Canvas API** | 基本的な画像処理               |
| **OpenCV.js**  | 高度な画像処理（必要に応じて） |

```typescript
// Canvas APIでの基本処理
const canvas = document.createElement('canvas');
const ctx = canvas.getContext('2d');
ctx.drawImage(image, 0, 0);
const imageData = ctx.getImageData(0, 0, width, height);
```

### フォント生成

| ライブラリ      | 用途        |
| --------------- | ----------- |
| **opentype.js** | TTF/OTF生成 |

```typescript
import opentype from 'opentype.js';

const font = new opentype.Font({
  familyName: 'MyHandwriting',
  styleName: 'Regular',
  unitsPerEm: 1000,
  ascender: 800,
  descender: -200,
});
```

---

## ホスティング

### Cloudflare Pages

| 項目             | 内容                         |
| ---------------- | ---------------------------- |
| 無料枠           | 500ビルド/月、無制限帯域     |
| CDN              | グローバルエッジネットワーク |
| HTTPS            | 自動                         |
| カスタムドメイン | 対応                         |

### デプロイフロー

```
GitHub Push
    ↓
Cloudflare Pages ビルド
    ↓
エッジにデプロイ
```

### Cloudflare Pagesセットアップ手順

1. **Cloudflareアカウント作成**
   - https://dash.cloudflare.com/sign-up でアカウント作成

2. **GitHubリポジトリ連携**
   - Cloudflare Dashboard → Pages → 「Create a project」
   - 「Connect to Git」→ GitHubアカウント連携
   - `kako-jun/my-font-craft` リポジトリを選択

3. **ビルド設定**
   | 項目 | 値 |
   |------|-----|
   | Framework preset | None |
   | Build command | `npm run build` |
   | Build output directory | `dist` |
   | Root directory | `/` |
   | Node.js version | 18.x |

4. **環境変数（必要に応じて）**
   - 本プロジェクトは環境変数不要（全処理クライアントサイド）

5. **デプロイ**
   - 「Save and Deploy」をクリック
   - 初回デプロイ完了後、`*.pages.dev` のURLが発行される

### カスタムドメイン設定（オプション）

1. Cloudflare Dashboard → Pages → プロジェクト → Custom domains
2. 「Set up a custom domain」
3. ドメインを入力（例: `myfontcraft.com`）
4. DNSレコードが自動設定される（Cloudflare DNS使用時）
5. SSL証明書は自動発行

### ローカルプレビュー

```bash
# Cloudflare Pages環境でローカル確認
npx wrangler pages dev dist
```

---

## 開発環境

### 必要なツール

| ツール  | バージョン |
| ------- | ---------- |
| Node.js | 18.x 以上  |
| npm     | 9.x 以上   |
| Git     | 最新       |

### セットアップ

```bash
# クローン
git clone https://github.com/kako-jun/my-font-craft.git
cd my-font-craft

# 依存関係インストール
npm install

# 開発サーバー起動
npm run dev

# ビルド
npm run build

# リント
npm run lint

# フォーマット
npm run format

# リント + フォーマットチェック
npm run check

# テスト（TypeScript: Vitest 20件）
npm test

# テスト（Rust CLI: 27件）
cd cli && cargo test

# デプロイ
npm run deploy
```

---

## ディレクトリ構成

```
my-font-craft/
├── cli/                     # Rust CLI（画像処理パイプライン）
│   ├── Cargo.toml
│   ├── generate-test-pdf.ts # テスト用PDF生成（layout.tsからimport）
│   └── src/
│       ├── main.rs          # CLIエントリーポイント（generate/process/distort）
│       ├── layout.rs        # レイアウト定数（layout.tsと同期）
│       ├── template.rs      # テンプレート画像生成
│       ├── pipeline.rs      # 10段階画像処理パイプライン
│       ├── marker.rs        # マーカー検出（union-find）
│       ├── perspective.rs   # ホモグラフィー変換
│       ├── qr.rs            # QRコード読み取り
│       ├── cell.rs          # セル切り出し+採用判定
│       └── distort.rs       # 合成歪みテスト用
├── docs/                    # ドキュメント
│   ├── README.md
│   ├── overview.md
│   ├── characters.md
│   ├── template-spec.md
│   ├── image-processing.md
│   ├── font-generation.md
│   ├── ui-design.md
│   ├── tech-stack.md
│   └── future-plans.md
├── .husky/
│   └── pre-commit           # lint-staged 実行
├── eslint.config.js         # ESLint v9 flat config
├── .prettierrc              # Prettier 設定
├── src/
│   ├── index.tsx            # Solid.jsエントリーポイント
│   ├── App.tsx              # ルートコンポーネント
│   ├── pages/               # ページコンポーネント
│   │   ├── Home.tsx
│   │   ├── Template.tsx
│   │   ├── Upload.tsx
│   │   └── About.tsx
│   ├── components/          # 共通コンポーネント
│   │   ├── Header.tsx
│   │   ├── Footer.tsx
│   │   ├── ProgressBar.tsx
│   │   └── ScanResultGrid.tsx
│   ├── lib/
│   │   ├── template/        # テンプレートPDF生成
│   │   │   ├── generator.ts
│   │   │   └── layout.ts
│   │   ├── scanner/         # 画像処理・文字切り出し
│   │   │   ├── processor.ts
│   │   │   ├── qr-reader.ts
│   │   │   └── marker-detector.ts
│   │   ├── vectorizer/      # ベクター化
│   │   │   └── contour.ts
│   │   └── font/            # フォント生成
│   │       ├── builder.ts
│   │       └── calt.ts
│   ├── data/
│   │   ├── characters.ts    # 文字リスト
│   │   └── joyo-kanji.ts    # 常用漢字リスト
│   └── styles/
│       └── global.css
├── tests/
│   ├── e2e/                 # E2Eテスト（Playwright）
│   │   └── full-flow.spec.ts
│   ├── unit/                # ユニットテスト（Vitest）
│   ├── integration/         # 結合テスト（Vitest）
│   ├── fixtures/            # テスト用フィクスチャ
│   └── helpers/             # テストヘルパー
├── public/                  # 静的ファイル
├── playwright.config.ts     # Playwright設定
├── README.md
├── CLAUDE.md
├── package.json
├── tsconfig.json
└── wrangler.toml            # Cloudflare設定
```

---

## パッケージ依存関係

### dependencies

```json
{
  "solid-js": "^1.9.0",
  "pdf-lib": "^1.17.1",
  "opentype.js": "^1.3.4",
  "jsqr": "^1.4.0",
  "jszip": "^3.10.1",
  "qrcode": "^1.5.4"
}
```

### devDependencies

```json
{
  "vite": "^6.0.0",
  "vite-plugin-solid": "^2.11.0",
  "typescript": "^5.7.0",
  "eslint": "^10.2.0",
  "@eslint/js": "^10.0.1",
  "@typescript-eslint/eslint-plugin": "^8.58.0",
  "@typescript-eslint/parser": "^8.58.0",
  "globals": "^16.0.0",
  "prettier": "^3.8.1",
  "husky": "^9.1.7",
  "lint-staged": "^16.4.0",
  "vitest": "^4.1.2",
  "@vitest/coverage-v8": "^4.1.2",
  "canvas": "^3.2.3",
  "wrangler": "^3.0.0"
}
```

---

## パフォーマンス最適化

### 将来の改善案

| 項目               | 手法                         |
| ------------------ | ---------------------------- |
| 画像処理高速化     | WebAssembly (Rust)           |
| 並列処理           | Web Worker                   |
| オフライン対応     | Service Worker               |
| バンドルサイズ削減 | Tree shaking, Code splitting |
