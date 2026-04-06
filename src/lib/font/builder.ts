import opentype from 'opentype.js';
import type { PathCommand } from '../vectorizer/contour';
import { generateCaltFeature } from './calt';

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
          path.bezierCurveTo(
            cmd.cp1x!, cmd.cp1y!,
            cmd.cp2x!, cmd.cp2y!,
            cmd.x, cmd.y,
          );
          break;
        case 'Z':
          path.closePath();
          break;
      }
    }
  }

  return path;
}
