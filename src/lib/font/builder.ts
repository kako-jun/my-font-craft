import opentype from 'opentype.js';
import type { PathCommand } from '../vectorizer/contour';
import type { GlyphStatus } from '../scanner/processor';

export interface VectorGlyph {
  name: string;
  unicode?: number;
  paths: PathCommand[][];
  advanceWidth: number;
}

export interface FontOptions {
  familyName: string;
  glyphs: VectorGlyph[];
  styleName?: string;
  version?: string;
  description?: string;
}

export async function buildFont(opts: FontOptions): Promise<ArrayBuffer> {
  const notdefPath = new opentype.Path();
  notdefPath.moveTo(100, 0);
  notdefPath.lineTo(100, 700);
  notdefPath.lineTo(600, 700);
  notdefPath.lineTo(600, 0);
  notdefPath.closePath();
  notdefPath.moveTo(150, 50);
  notdefPath.lineTo(550, 50);
  notdefPath.lineTo(550, 650);
  notdefPath.lineTo(150, 650);
  notdefPath.closePath();

  const notdefGlyph = new opentype.Glyph({
    name: '.notdef',
    advanceWidth: 700,
    path: notdefPath,
  });

  // スペースグリフ
  const spacePath = new opentype.Path();
  const spaceGlyph = new opentype.Glyph({
    name: 'space',
    unicode: 32,
    advanceWidth: 500,
    path: spacePath,
  });

  const glyphs: opentype.Glyph[] = [notdefGlyph, spaceGlyph];

  for (const vg of opts.glyphs) {
    const path = convertToOpentypePath(vg.paths);
    const glyph = new opentype.Glyph({
      name: vg.name,
      unicode: vg.unicode,
      advanceWidth: vg.advanceWidth,
      path,
    });
    glyphs.push(glyph);
  }

  const font = new opentype.Font({
    familyName: opts.familyName,
    styleName: opts.styleName || 'Regular',
    unitsPerEm: 1000,
    ascender: 800,
    descender: -200,
    glyphs,
  });

  // calt（Contextual Alternates）設定
  // NOTE: opentype.js v1.x の substitution API は限定的。
  // バリエーショングリフはフォントに含まれるが、calt ルールの自動適用は
  // 将来的に opentype.js v2 または別ライブラリへの移行で対応予定。
  // 現時点ではバリエーショングリフを .alt 名で登録し、
  // 対応アプリから手動でアクセスできる状態にする。

  return font.toArrayBuffer();
}

/**
 * 既存TTF/OTFファイルを読み込み、グリフとステータスを返す
 */
export function importFont(buffer: ArrayBuffer): {
  glyphs: VectorGlyph[];
  statuses: GlyphStatus[];
} {
  const font = opentype.parse(buffer);
  const glyphs: VectorGlyph[] = [];
  const statuses: GlyphStatus[] = [];

  for (let i = 0; i < font.glyphs.length; i++) {
    const glyph = font.glyphs.get(i);

    // .notdef と space をスキップ
    if (glyph.name === '.notdef' || glyph.unicode === undefined || glyph.unicode === null) continue;
    if (glyph.unicode === 32) continue;

    const paths = convertFromOpentypePath(glyph.path);
    const unicode = glyph.unicode;
    const char = String.fromCodePoint(unicode);
    const name = `uni${unicode.toString(16).toUpperCase().padStart(4, '0')}`;

    // サムネイル用 data URL を生成
    let cellImageDataUrl: string | undefined;
    if (typeof document !== 'undefined')
      try {
        const canvas = document.createElement('canvas');
        const size = 80;
        canvas.width = size;
        canvas.height = size;
        const ctx = canvas.getContext('2d');
        if (!ctx) throw new Error('2d context unavailable');

        // グリフをキャンバスに描画
        const path = glyph.getPath(4, size - 8, size - 16);
        const pathData = path.toSVG(2); // undocumented decimal places param
        // Path2D + SVG path data で描画
        const match = /\bd="([^"]+)"/.exec(pathData);
        if (match) {
          const p2d = new Path2D(match[1]);
          ctx.fillStyle = '#333';
          ctx.fill(p2d);
        }

        cellImageDataUrl = canvas.toDataURL('image/png');
      } catch {
        /* Canvas未対応環境ではスキップ */
      }

    glyphs.push({
      name,
      unicode,
      paths,
      advanceWidth: glyph.advanceWidth ?? 1000,
    });

    statuses.push({
      char,
      unicode,
      pageIndex: 0,
      row: 0,
      col: 0,
      status: 'imported',
      cellImageDataUrl,
    });
  }

  return { glyphs, statuses };
}

/**
 * opentype.js の path を内部の PathCommand[][] に逆変換する
 */
function convertFromOpentypePath(otPath: opentype.Path): PathCommand[][] {
  const commands = otPath.commands;
  if (commands.length === 0) return [];

  const pathGroups: PathCommand[][] = [];
  let currentGroup: PathCommand[] = [];
  let lastX = 0;
  let lastY = 0;

  // opentype.js の Y 座標はフォント座標系（上が正）
  // 内部の PathCommand も同じ座標系を使う（builder.ts で変換済み）
  for (const cmd of commands) {
    switch (cmd.type) {
      case 'M':
        if (currentGroup.length > 0) {
          pathGroups.push(currentGroup);
          currentGroup = [];
        }
        currentGroup.push({ type: 'M', x: cmd.x, y: cmd.y });
        lastX = cmd.x;
        lastY = cmd.y;
        break;
      case 'L':
        currentGroup.push({ type: 'L', x: cmd.x, y: cmd.y });
        lastX = cmd.x;
        lastY = cmd.y;
        break;
      case 'Q': {
        // Quadratic → Cubic 変換
        // cp1 = start + 2/3 * (control - start)
        // cp2 = end + 2/3 * (control - end)
        const qx = cmd.x1;
        const qy = cmd.y1;
        const cp1x = lastX + (2 / 3) * (qx - lastX);
        const cp1y = lastY + (2 / 3) * (qy - lastY);
        const cp2x = cmd.x + (2 / 3) * (qx - cmd.x);
        const cp2y = cmd.y + (2 / 3) * (qy - cmd.y);
        currentGroup.push({
          type: 'C',
          x: cmd.x,
          y: cmd.y,
          cp1x,
          cp1y,
          cp2x,
          cp2y,
        });
        lastX = cmd.x;
        lastY = cmd.y;
        break;
      }
      case 'C':
        currentGroup.push({
          type: 'C',
          x: cmd.x,
          y: cmd.y,
          cp1x: cmd.x1,
          cp1y: cmd.y1,
          cp2x: cmd.x2,
          cp2y: cmd.y2,
        });
        lastX = cmd.x;
        lastY = cmd.y;
        break;
      case 'Z':
        currentGroup.push({ type: 'Z', x: 0, y: 0 });
        pathGroups.push(currentGroup);
        currentGroup = [];
        break;
    }
  }

  if (currentGroup.length > 0) {
    pathGroups.push(currentGroup);
  }

  return pathGroups;
}

function convertToOpentypePath(pathGroups: PathCommand[][]): opentype.Path {
  const path = new opentype.Path();

  for (const commands of pathGroups) {
    for (const cmd of commands) {
      switch (cmd.type) {
        case 'M':
          path.moveTo(cmd.x, cmd.y);
          break;
        case 'L':
          path.lineTo(cmd.x, cmd.y);
          break;
        case 'C':
          path.bezierCurveTo(cmd.cp1x!, cmd.cp1y!, cmd.cp2x!, cmd.cp2y!, cmd.x, cmd.y);
          break;
        case 'Z':
          path.closePath();
          break;
      }
    }
  }

  return path;
}
