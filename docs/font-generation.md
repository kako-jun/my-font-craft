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
