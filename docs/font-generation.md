# フォント生成仕様

## 出力形式

- TrueType (.ttf) — Phase 1 で対応
- OpenType (.otf) — 将来対応予定

---

## フォントメトリクス

| 項目 | 値 |
|------|-----|
| Units per Em | 1000 |
| Ascender | 800 |
| Descender | -200 |
| Line Gap | 0 |
| Cap Height | 700 |
| x-Height | 500 |

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
  advanceWidth: 1000
};
```

### 命名規則

| 種類 | 名前 | 例 |
|------|------|-----|
| 基本グリフ | `uni{XXXX}` | `uni3042`（あ） |
| バリエーション1 | `uni{XXXX}.alt1` | `uni3042.alt1` |
| バリエーション2 | `uni{XXXX}.alt2` | `uni3042.alt2` |

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
  descender: -200
});

// グリフを追加
for (const char of characters) {
  const path = convertToOpentypePath(char.vectorData);
  const glyph = new opentype.Glyph({
    name: `uni${char.unicode.toString(16).toUpperCase()}`,
    unicode: char.unicode,
    advanceWidth: 1000,
    path: path
  });
  font.glyphs.push(glyph);
}

// フォントを出力
const arrayBuffer = font.toArrayBuffer();
```

### パスの変換

ベクター化されたデータをopentype.jsのPathオブジェクトに変換:

```typescript
function convertToOpentypePath(vectorData: VectorData): opentype.Path {
  const path = new opentype.Path();

  for (const contour of vectorData.contours) {
    path.moveTo(contour[0].x, contour[0].y);

    for (let i = 1; i < contour.length; i++) {
      const point = contour[i];
      if (point.type === 'line') {
        path.lineTo(point.x, point.y);
      } else if (point.type === 'curve') {
        path.bezierCurveTo(
          point.cp1x, point.cp1y,
          point.cp2x, point.cp2y,
          point.x, point.y
        );
      }
    }

    path.closePath();
  }

  return path;
}
```

---

## フォントメタデータ

### 必須項目

| 項目 | 値 |
|------|-----|
| familyName | ユーザー指定 or "MyHandwriting" |
| styleName | "Regular" |
| version | "1.0" |
| copyright | ユーザー指定 or 空欄 |

### オプション項目

| 項目 | 値 |
|------|-----|
| designer | ユーザー指定 |
| description | "Created with MyFontCraft" |
| license | ユーザー指定 |

---

## パフォーマンス目標

| 処理 | 目標時間 |
|------|---------|
| フォント生成（2,400グリフ） | 10秒以内 |
| ファイル出力 | 1秒以内 |

---

## 出力ファイル

### ファイル名

デフォルト: `MyHandwriting.ttf` または `MyHandwriting.otf`

ユーザーがフォント名を指定した場合: `{FontName}.ttf`

### ファイルサイズ目安

- 約2,400グリフ
- 推定サイズ: 2〜5MB（ベクターの複雑さによる）
