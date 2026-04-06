import { PDFDocument, rgb, StandardFonts } from 'pdf-lib';
import QRCode from 'qrcode';
import {
  mm,
  PAGE_WIDTH,
  PAGE_HEIGHT,
  COLS,
  CELL_SIZE,
  INNER_SIZE,
  CHECK_HEIGHT,
  QR_X,
  QR_Y,
  QR_SIZE,
  GRAY_BAR_X,
  GRAY_BAR_Y,
  GRAY_BAR_STEP_W,
  GRAY_BAR_STEP_H,
  GRAY_BAR_STEPS,
  CYAN_SAMPLE_X,
  CYAN_SAMPLE_Y,
  CYAN_SAMPLE_SIZE,
  MARKERS,
  MARGIN,
  MARKER_SIZE,
  SAMPLE_WIDTH,
  COLOR_CYAN,
  getCellPosition,
  getSamplePosition,
} from './layout';
import {
  HIRAGANA,
  KATAKANA,
  UPPERCASE,
  LOWERCASE,
  DIGITS,
  ASCII_SYMBOLS,
  JP_SYMBOLS,
  CHARS_PER_PAGE,
} from '../../data/characters';
import { JOYO_KANJI } from '../../data/joyo-kanji';

export interface TemplateOptions {
  fontName: string;
  includeHiragana: boolean;
  includeKatakana: boolean;
  includeKanji: boolean;
  includeAlphaNum: boolean;
}

function buildCharList(opts: TemplateOptions): string[] {
  const chars: string[] = [];
  if (opts.includeHiragana) chars.push(...HIRAGANA);
  if (opts.includeKatakana) chars.push(...KATAKANA);
  if (opts.includeAlphaNum)
    chars.push(...UPPERCASE, ...LOWERCASE, ...DIGITS, ...ASCII_SYMBOLS, ...JP_SYMBOLS);
  if (opts.includeKanji) chars.push(...JOYO_KANJI);
  return chars;
}

// 任意の文字リストからリトライ用テンプレートPDFを生成
// QRコードに文字リストを埋め込み、スキャン時にページ番号ではなく文字リストで識別
export async function generateRetryTemplatePDF(
  chars: string[],
  fontName: string,
): Promise<Uint8Array> {
  return generateTemplatePDFFromChars(chars, fontName, true);
}

export async function generateTemplatePDF(opts: TemplateOptions): Promise<Uint8Array> {
  const chars = buildCharList(opts);
  return generateTemplatePDFFromChars(chars, opts.fontName, false);
}

async function generateTemplatePDFFromChars(
  chars: string[],
  fontName: string,
  includeCharsInQR = false,
): Promise<Uint8Array> {
  const totalPages = Math.ceil(chars.length / CHARS_PER_PAGE);
  const pdfDoc = await PDFDocument.create();

  // フォント埋め込み（見本文字用 — ASCII のみ対応、日本語は後で対処）
  const helvetica = await pdfDoc.embedFont(StandardFonts.Helvetica);

  for (let pageIdx = 0; pageIdx < totalPages; pageIdx++) {
    const page = pdfDoc.addPage([mm(PAGE_WIDTH), mm(PAGE_HEIGHT)]);
    const pageChars = chars.slice(pageIdx * CHARS_PER_PAGE, (pageIdx + 1) * CHARS_PER_PAGE);

    // PDF座標系は左下原点なので変換ヘルパー
    const toY = (mmY: number) => mm(PAGE_HEIGHT) - mm(mmY);

    // --- ヘッダー ---
    // タイトル
    page.drawText(`MyFontCraft`, {
      x: mm(25),
      y: toY(14),
      size: 10,
      font: helvetica,
      color: rgb(0, 0, 0),
    });

    // ページ番号（右寄せ）
    const pageNumText = `Page ${pageIdx + 1} / ${totalPages}`;
    const pageNumWidth = helvetica.widthOfTextAtSize(pageNumText, 9);
    const rightEdge = mm(PAGE_WIDTH - MARGIN); // 右余白の内側
    page.drawText(pageNumText, {
      x: rightEdge - pageNumWidth,
      y: toY(14),
      size: 9,
      font: helvetica,
      color: rgb(0, 0, 0),
    });

    // QRコード
    const qrPayload: Record<string, unknown> = {
      p: 'mfc',
      v: 1,
      pg: pageIdx + 1,
      t: totalPages,
      m: 2,
    };
    // リトライ用テンプレートでは文字リストをQRに埋め込む
    if (includeCharsInQR) {
      qrPayload.chars = pageChars;
    }
    const qrData = JSON.stringify(qrPayload);
    try {
      const qrDataUrl = await QRCode.toDataURL(qrData, {
        errorCorrectionLevel: 'M',
        margin: 0,
        width: 128,
      });
      const qrImageBytes = Uint8Array.from(atob(qrDataUrl.split(',')[1]), (c) => c.charCodeAt(0));
      const qrImage = await pdfDoc.embedPng(qrImageBytes);
      page.drawImage(qrImage, {
        x: mm(QR_X),
        y: toY(QR_Y + QR_SIZE),
        width: mm(QR_SIZE),
        height: mm(QR_SIZE),
      });
    } catch {
      // QRコード生成失敗時はスキップ（データが大きすぎる場合など）
    }

    // グレースケールバー（左が100%黒→右が10%グレー）
    for (let i = 0; i < GRAY_BAR_STEPS; i++) {
      const intensity = i / GRAY_BAR_STEPS;
      page.drawRectangle({
        x: mm(GRAY_BAR_X + i * GRAY_BAR_STEP_W),
        y: toY(GRAY_BAR_Y + GRAY_BAR_STEP_H),
        width: mm(GRAY_BAR_STEP_W),
        height: mm(GRAY_BAR_STEP_H),
        color: rgb(intensity, intensity, intensity),
      });
    }

    // シアンサンプル
    page.drawRectangle({
      x: mm(CYAN_SAMPLE_X),
      y: toY(CYAN_SAMPLE_Y + CYAN_SAMPLE_SIZE),
      width: mm(CYAN_SAMPLE_SIZE),
      height: mm(CYAN_SAMPLE_SIZE),
      color: rgb(COLOR_CYAN.r, COLOR_CYAN.g, COLOR_CYAN.b),
    });

    // --- 四隅マーカー ---
    for (const [, marker] of Object.entries(MARKERS)) {
      drawStarMarker(page, mm(marker.x), toY(marker.y), mm(MARKER_SIZE), marker.filled);
    }

    // --- 本文：文字マス ---
    for (let i = 0; i < pageChars.length; i++) {
      const row = Math.floor(i / COLS);
      const col = i % COLS;
      const char = pageChars[i];

      // 見本文字（Canvas APIで描画→画像としてPDFに埋め込み）
      const sample = getSamplePosition(row, col);
      const charCode = char.charCodeAt(0);
      if (charCode < 128) {
        // ASCII文字はフォントで直接描画
        page.drawText(char, {
          x: mm(sample.x + 2),
          y: toY(sample.y + 14),
          size: 12,
          font: helvetica,
          color: rgb(0, 0, 0),
        });
      } else {
        // 日本語文字はCanvas→PNG→PDF埋め込み
        const charImage = await renderCharToImage(char);
        if (charImage) {
          const pngImage = await pdfDoc.embedPng(charImage);
          const charSize = SAMPLE_WIDTH;
          const charY = sample.y + (CELL_SIZE - charSize) / 2;
          page.drawImage(pngImage, {
            x: mm(sample.x),
            y: toY(charY + charSize),
            width: mm(charSize),
            height: mm(charSize),
          });
        }
      }

      // 2つのマス
      for (let cellIdx = 0; cellIdx < 2; cellIdx++) {
        const pos = getCellPosition(row, col, cellIdx);

        // 外枠（黒）
        page.drawRectangle({
          x: mm(pos.x),
          y: toY(pos.y + CELL_SIZE),
          width: mm(CELL_SIZE),
          height: mm(CELL_SIZE),
          borderColor: rgb(0, 0, 0),
          borderWidth: 1,
        });

        // 内枠（シアン）
        const innerOffset = (CELL_SIZE - INNER_SIZE) / 2;
        page.drawRectangle({
          x: mm(pos.x + innerOffset),
          y: toY(pos.y + innerOffset + INNER_SIZE),
          width: mm(INNER_SIZE),
          height: mm(INNER_SIZE),
          borderColor: rgb(COLOR_CYAN.r, COLOR_CYAN.g, COLOR_CYAN.b),
          borderWidth: 0.5,
        });

        // チェック欄区切り（シアン）
        page.drawLine({
          start: { x: mm(pos.x), y: toY(pos.y + CELL_SIZE + CHECK_HEIGHT) },
          end: { x: mm(pos.x + CELL_SIZE), y: toY(pos.y + CELL_SIZE + CHECK_HEIGHT) },
          color: rgb(COLOR_CYAN.r, COLOR_CYAN.g, COLOR_CYAN.b),
          thickness: 0.5,
        });

        // チェック欄外枠
        page.drawRectangle({
          x: mm(pos.x),
          y: toY(pos.y + CELL_SIZE + CHECK_HEIGHT),
          width: mm(CELL_SIZE),
          height: mm(CHECK_HEIGHT),
          borderColor: rgb(0, 0, 0),
          borderWidth: 0.5,
        });

        // チェック欄にシアンで✓サンプル（スキャン時に除去される）
        const checkY = pos.y + CELL_SIZE;
        const cx = pos.x + 3;
        const cy = checkY + CHECK_HEIGHT / 2;
        // ✓ を2本の線で描画
        page.drawLine({
          start: { x: mm(cx), y: toY(cy) },
          end: { x: mm(cx + 2), y: toY(cy + 1.2) },
          color: rgb(COLOR_CYAN.r, COLOR_CYAN.g, COLOR_CYAN.b),
          thickness: 0.8,
        });
        page.drawLine({
          start: { x: mm(cx + 2), y: toY(cy + 1.2) },
          end: { x: mm(cx + 5), y: toY(cy - 1) },
          color: rgb(COLOR_CYAN.r, COLOR_CYAN.g, COLOR_CYAN.b),
          thickness: 0.8,
        });
      }
    }
  }

  return pdfDoc.save();
}

// 日本語文字をCanvas APIで描画してPNG画像のバイト列を返す
async function renderCharToImage(char: string): Promise<Uint8Array | null> {
  if (typeof document === 'undefined') return null;
  const size = 64;
  const canvas = document.createElement('canvas');
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;

  ctx.fillStyle = '#FFFFFF';
  ctx.fillRect(0, 0, size, size);
  ctx.fillStyle = '#000000';
  ctx.font = `${size * 0.75}px serif`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(char, size / 2, size / 2);

  const dataUrl = canvas.toDataURL('image/png');
  const base64 = dataUrl.split(',')[1];
  return Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
}

// ドット絵風の星マーカー（簡易版：四角+十字で星っぽく）
function drawStarMarker(
  page: ReturnType<PDFDocument['addPage']>,
  x: number,
  y: number,
  size: number,
  filled: boolean,
) {
  const unit = size / 5;
  const c = rgb(0, 0, 0);

  if (filled) {
    // 塗りつぶし十字
    page.drawRectangle({
      x: x + unit,
      y: y - size + unit,
      width: unit * 3,
      height: size,
      color: c,
    });
    page.drawRectangle({ x, y: y - unit * 3, width: size, height: unit * 3, color: c });
  } else {
    // 枠線のみ十字
    page.drawRectangle({
      x: x + unit,
      y: y - size + unit,
      width: unit * 3,
      height: size,
      borderColor: c,
      borderWidth: 0.5,
    });
    page.drawRectangle({
      x,
      y: y - unit * 3,
      width: size,
      height: unit * 3,
      borderColor: c,
      borderWidth: 0.5,
    });
  }
}
