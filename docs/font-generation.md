# フォント生成仕様

## 出力形式

- TrueType (.ttf) — Phase 1 で対応
- OpenType (.otf) — 将来対応予定

---

## フォントメトリクス

| 項目         | 値   |
| ------------ | ---- |
| Units per Em | 1000 |
| Ascender     | 800  |
| Descender    | -200 |
| Line Gap     | 0    |
| Cap Height   | 700  |
| x-Height     | 500  |

---

## グリフ配置: セル→em 固定変換（#111）

### 原則

**書き手がセル内のどこに・どの大きさで書いたかを、そのままフォントに出す。**
per-glyph の bbox 正規化（#53 の「bbox を 750 units に拡大して em 中央に配置」）は
位置・大きさ情報を捨てるため廃止した。bbox 正規化では「、」「。」が「あ」と同じ
大きさで行中央に浮き、小書きかな「っゃゅょ」が等倍化され、offset_y ≥ 0 のため
descender（g/j/p/q/y のベースライン下突出）が原理的に表現できなかった（#108 原因1）。

### 座標系

テンプレートの**内枠（10mm 角、書く領域）を em-square とみなす**固定アフィン変換:

| 項目               | 値                                            |
| ------------------ | --------------------------------------------- |
| スケール           | 1mm = 100 units（= 1000 units / 10mm）        |
| 内枠左端           | x = 0                                         |
| 内枠右端           | x = 1000                                      |
| 内枠下端           | y = **-120**（EMBOX_BOTTOM_Y）                |
| 内枠上端           | y = 880                                       |
| ベースライン (y=0) | 内枠下端の **1.2mm 上**（= ガイド線の位置）   |
| セル crop 全域     | x ∈ [-100, 1100], y ∈ [-220, 980]（12mm四方） |

```
  em Y
  880 ┌─────────────────┐ ← 内枠上端（ascender 800 の少し上）
      │                 │
      │   あ・漢字は     │
      │   中央に大きく   │
      │                 │
    0 ├──── baseline ───┤ ← ガイド線（内枠下端の1.2mm上）。英字はここに乗せる
 -120 └─────────────────┘ ← 内枠下端。g/j/p/q/y の尻尾・「、」はこの帯に届く
 -200   (Descender)
```

### 内枠下端 = -120 の根拠

- CJK フォントの ideographic embox 慣例（1000 upm でembox 下端 ≈ -120、上端 ≈ 880）に
  合わせる。市販の日本語フォントと混植したときにかな・漢字の視覚的な行位置が揃う
- フォントメトリクスとの整合: descender(-200) の**内側**に収まるので、内枠いっぱいに
  書いてもメトリクス外にはみ出さない。上端 880 は ascender(800) を 80 超えるが、
  これは「行間に食い込み得る余白」であり手書きフォントでは許容（通常のかな・漢字は
  内枠に対して自然な余白を持って書かれるため実インクは 800 以下に収まる）
- ベースライン位置 1.2mm（= 120 units / 100 units/mm）はテンプレートのベースライン
  ガイド線と厳密に一致する（関係式は `cli/src/vectorizer.rs` のテスト
  `baseline_guide_maps_to_em_zero` で固定）

### 効果

- 「、」「。」→ 左下に小さく（書いたとおり）
- 小書きかな「っゃゅょぁぃぅぇぉ」→ 小さいまま
- g/j/p/q/y → ベースラインガイドに乗せて書けば尻尾が y<0 の descender 領域に出る
- 長音「ー」→ 横長の細い棒のまま（正方形に拡大されない）

### セル crop との関係

セル切り出しは外枠から `CELL_CROP_MARGIN`(1.5mm) 内側の 12mm 四方
（`cli/src/layout.rs` の `CELL_CROP_MARGIN` / `CELL_CROP_SIZE` が正本）。
固定変換はこの crop の物理寸法を前提にしているため、crop マージンを変える場合は
必ず layout 定数を通す。内枠の外（crop 内の余白 1mm）に書いた分は em の外
（x<0, x>1000 等）へそのまま写る = はみ出しもユーザーの字として保存される。

### bbox 正規化は opt-in の救済（既定 OFF）

旧方式は `vectorize_binary_bbox_fit()` として判断ロジックのみ保存している。
「セルに対して明らかに小さすぎる字を後から拡大したい」ケース専用で、
**既定 OFF・プロダクション経路からの呼び出しなし**（UI 配線も未実装）。
有効化する場合も文字クラス（かな・漢字のみ等）を限定しないと句読点・小書きかなを
再び破壊することに注意。

### advanceWidth（送り幅）

全グリフ **1000 固定（全角モノスペース）** を維持する。日本語かな・漢字は全角で
正しく、英数字は当面「全角英数」の見た目になる。

**段階案（記録のみ、未実装）** — 英数字のプロポーショナル化:

1. Phase A: 英数字（U+0020〜U+007E）のみ、グリフ実インクの x 範囲から
   `advanceWidth = (x_max - x_min) + LSB + RSB`（サイドベアリングは固定値、例 50 units）
   を算出。かな・漢字・記号は 1000 固定のまま
2. Phase B: LSB/RSB を文字クラス別に調整（i/l と m/w で違和感が出るため）
3. 検討事項: 空白セルから生成されないスペース幅、数字の等幅性（表組み用途では
   数字だけ等幅が望ましい）、既存 TTF インポートグリフとの混在

---

## グリフ登録

### 基本グリフ

各文字に対してUnicodeコードポイントでグリフを登録。

```typescript
// 例: ひらがな「あ」
const glyph = {
  name: 'uni3042',
  unicode: 0x3042,
  path: vectorizedPath,
  advanceWidth: 1000,
};
```

### 命名規則

| 種類            | 名前             | 例              |
| --------------- | ---------------- | --------------- |
| 基本グリフ      | `uni{XXXX}`      | `uni3042`（あ） |
| バリエーション1 | `uni{XXXX}.alt1` | `uni3042.alt1`  |
| バリエーション2 | `uni{XXXX}.alt2` | `uni3042.alt2`  |

---

## 複数バリエーション対応

### OpenType Contextual Alternates (calt)

同じ文字が連続した場合に自動的に別のグリフを表示する機能。

#### 仕組み

```
入力: ああああ
出力: あ あ(alt1) あ あ(alt1)
      ↑ 基本    ↑ バリエーション（交互）
```

#### calt機能コード（例）

```opentype
feature calt {
  # 「あ」の後に「あ」が来たら、後者をalt1に置換
  sub uni3042 uni3042' by uni3042.alt1;

  # 「あ(alt1)」の後に「あ」が来たら、後者は基本のまま
  sub uni3042.alt1 uni3042' by uni3042;
} calt;
```

#### 対応アプリケーション

- Microsoft Word
- Adobe InDesign / Illustrator
- Webブラウザ（CSS: `font-feature-settings: "calt" 1;`）
- テキストエディタ（多くの場合デフォルトON）

---

## opentype.jsでの実装

### フォント作成

```typescript
import opentype from 'opentype.js';

const font = new opentype.Font({
  familyName: 'MyHandwriting',
  styleName: 'Regular',
  unitsPerEm: 1000,
  ascender: 800,
  descender: -200,
});

// グリフを追加
for (const char of characters) {
  const path = convertToOpentypePath(char.vectorData);
  const glyph = new opentype.Glyph({
    name: `uni${char.unicode.toString(16).toUpperCase()}`,
    unicode: char.unicode,
    advanceWidth: 1000,
    path: path,
  });
  font.glyphs.push(glyph);
}

// フォントを出力
const arrayBuffer = font.toArrayBuffer();
```

### パスの変換

Rust 側（`cli/src/vectorizer.rs`）がランレングス方式で生成した `PathCommand[][]` を
opentype.js の Path オブジェクトに変換する。各パスは矩形（M→L→L→L→Z）で、
二値化→2x アップスケール後の黒ピクセル連続区間に対応する。

```typescript
// builder.ts での実際の変換
for (const sub of glyph.paths) {
  for (const cmd of sub) {
    switch (cmd.type) {
      case 'M':
        path.moveTo(cmd.x, cmd.y);
        break;
      case 'L':
        path.lineTo(cmd.x, cmd.y);
        break;
      case 'C':
        path.bezierCurveTo(cmd.cp1x, cmd.cp1y, cmd.cp2x, cmd.cp2y, cmd.x, cmd.y);
        break;
      case 'Z':
        path.closePath();
        break;
    }
  }
}
```

---

## 既存フォントのインポート

### 概要

既存の TTF/OTF ファイルを読み込み、グリフを内部形式（`VectorGlyph[]`）に逆変換する。
スキャン結果と同じ review グリッドに `imported` ステータスで合流する。

### パス逆変換

opentype.js の `parse()` で取得したグリフのパスコマンドを内部の `PathCommand[][]` に変換:

- `M` / `L` / `C` / `Z` → そのまま対応
- `Q`（二次ベジェ）→ `C`（三次ベジェ）に変換
  - `cp1 = start + 2/3 * (control - start)`
  - `cp2 = end + 2/3 * (control - end)`

### マージ優先順位

| ステータス | 由来     | 優先度             |
| ---------- | -------- | ------------------ |
| `found`    | スキャン | 高（手書きが優先） |
| `imported` | 既存TTF  | 中                 |
| `empty`    | 未検出   | 低                 |

#### マージ仕様（Issue #93）

由来は `GlyphStatus.status` フィールドで識別する（新規メタフィールドは追加しない）:

- **画像（スキャン）→ 何でも**: 後勝ち。新しい `found` は既存の `found` / `imported` / `empty` を全て上書きする。同 unicode の旧 alt-variant も合わせて破棄し、新スキャン側の alt-variant は採用する
- **TTF インポート → 既存**: 画像由来 (`found`) は守る。`empty` / 既存 `imported` は新しい `imported` で置き換える（TTF同士は後勝ち）
- **scanner 内部の重複排除**: 同一アップロード内で同じ unicode が複数回検出された場合（複数ページに同じ字、複数画像が同じ字を含む等）、ベースグリフは Map で後勝ち、alt は対応する旧 alt を破棄してから追加する

#### 入力モード一覧（Issue #97）

| 入力                | 複数選択 | マージ挙動                                          |
| ------------------- | -------- | --------------------------------------------------- |
| 画像（image/\*）    | 可       | 既存があれば常に追加（merge）                       |
| フォルダ            | 単数     | 既存があれば常に追加（merge）                       |
| ZIP                 | 単数     | 既存があれば常に追加（merge）                       |
| 既存フォント (.ttf) | 単数     | `empty` を埋める / TTF同士は後勝ち / 画像由来は守る |

リセットしたい場合は UI 上の「リセット（0 文字に戻す）」ボタンを明示的に押す。

---

## フォントメタデータ

### 必須項目

| 項目       | 値                              |
| ---------- | ------------------------------- |
| familyName | ユーザー指定 or "MyHandwriting" |
| styleName  | "Regular"                       |
| version    | "1.0"                           |
| copyright  | ユーザー指定 or 空欄            |

### オプション項目

| 項目        | 値                         |
| ----------- | -------------------------- |
| designer    | ユーザー指定               |
| description | "Created with MyFontCraft" |
| license     | ユーザー指定               |

---

## パフォーマンス目標

| 処理                        | 目標時間 |
| --------------------------- | -------- |
| フォント生成（2,400グリフ） | 10秒以内 |
| ファイル出力                | 1秒以内  |

---

## 出力ファイル

### ファイル名

デフォルト: `MyHandwriting.ttf` または `MyHandwriting.otf`

ユーザーがフォント名を指定した場合: `{FontName}.ttf`

### ファイルサイズ目安

- 約2,400グリフ
- 推定サイズ: 2〜5MB（ベクターの複雑さによる）
