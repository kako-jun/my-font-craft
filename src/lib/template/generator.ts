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
  GRAY_BAR_STEPS,
  GRAY_BAR_STEP_SIZE,
  GRAY_BAR_LEFT_X,
  GRAY_BAR_RIGHT_X,
  GRAY_BAR_TOP_Y,
  GRAY_BAR_BOTTOM_Y,
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
import { getTriviaForPage } from '../../data/trivia';

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
    // タイトル + フォント名（1行にまとめて同じフォント・サイズで描画）
    const titleText = fontName ? `MyFontCraft — "${fontName}"` : 'MyFontCraft';
    // ASCII文字はHelveticaで直接描画
    const titleIsAscii = /^[\x20-\x7E]*$/.test(titleText);
    if (titleIsAscii) {
      page.drawText(titleText, {
        x: mm(25),
        y: toY(14),
        size: 10,
        font: helvetica,
        color: rgb(0, 0, 0),
      });
    } else {
      // 日本語フォント名の場合: タイトル部分はHelvetica、フォント名はCanvas→PNG
      page.drawText('MyFontCraft', {
        x: mm(25),
        y: toY(14),
        size: 10,
        font: helvetica,
        color: rgb(0, 0, 0),
      });
      const fnLabel = `— "${fontName}"`;
      const fnImage = await renderTextToImage(fnLabel, 8);
      if (fnImage) {
        const fnEmbed = await pdfDoc.embedPng(fnImage.data);
        // タイトル幅を測定して直後に配置
        const titleWidth = helvetica.widthOfTextAtSize('MyFontCraft ', 10);
        const fnHeightMm = 3.5;
        const fnWidthMm = (fnImage.widthPx / fnImage.heightPx) * fnHeightMm;
        page.drawImage(fnEmbed, {
          x: mm(25) + titleWidth,
          y: toY(14.5),
          width: mm(Math.min(fnWidthMm, 80)),
          height: mm(fnHeightMm),
        });
      }
    }

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

    // 雑学コメント（ヘッダー2行目、Canvas→PNG→PDF埋め込み）
    const triviaText = getTriviaForPage(pageIdx, pageChars, totalPages);
    const triviaImage = await renderTextToImage(triviaText, 6);
    if (triviaImage) {
      const triviaEmbed = await pdfDoc.embedPng(triviaImage.data);
      // アスペクト比を保って高さ4mmに収める
      const triviaHeightMm = 4;
      const triviaWidthMm = (triviaImage.widthPx / triviaImage.heightPx) * triviaHeightMm;
      // 右寄せ: ページ番号の右端に揃える。右上マーカー(x=192)と重ならないよう制限
      const maxWidthMm = 192 - MARGIN - 25; // マーカー左端(192) - 左余白(10) - タイトル領域(25) = 157mm
      const clampedWidthMm = Math.min(triviaWidthMm, maxWidthMm);
      const clampedHeightMm =
        clampedWidthMm < triviaWidthMm
          ? triviaHeightMm * (clampedWidthMm / triviaWidthMm)
          : triviaHeightMm;
      // ページ番号(14mm)の下に配置。画像の上端が17mmになるよう下端を計算
      const triviaTopMm = 17;
      const triviaBottomMm = triviaTopMm + clampedHeightMm;
      // 右上マーカー(x=192)の左に収まるよう配置
      const triviaRightEdge = mm(190); // マーカーとの間に2mm余白
      page.drawImage(triviaEmbed, {
        x: triviaRightEdge - mm(clampedWidthMm),
        y: toY(triviaBottomMm),
        width: mm(clampedWidthMm),
        height: mm(clampedHeightMm),
      });
    }

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

    // 左右縦グレースケールバー
    const barHeight = GRAY_BAR_BOTTOM_Y - GRAY_BAR_TOP_Y;
    const stepHeight = barHeight / GRAY_BAR_STEPS;
    for (let i = 0; i < GRAY_BAR_STEPS; i++) {
      const intensity = i / GRAY_BAR_STEPS; // 0=黒, 0.9=ほぼ白
      const y = GRAY_BAR_TOP_Y + i * stepHeight;
      // 左バー
      page.drawRectangle({
        x: mm(GRAY_BAR_LEFT_X),
        y: toY(y + stepHeight),
        width: mm(GRAY_BAR_STEP_SIZE),
        height: mm(stepHeight),
        color: rgb(intensity, intensity, intensity),
      });
      // 右バー
      page.drawRectangle({
        x: mm(GRAY_BAR_RIGHT_X),
        y: toY(y + stepHeight),
        width: mm(GRAY_BAR_STEP_SIZE),
        height: mm(stepHeight),
        color: rgb(intensity, intensity, intensity),
      });
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
          borderWidth: 0.5,
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
        const checkMarkWidth = 5;
        const cx = pos.x + (CELL_SIZE - checkMarkWidth) / 2;
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

// テキスト（日本語対応）をCanvas APIで描画してPNG画像のバイト列を返す
async function renderTextToImage(
  text: string,
  fontSizePt: number,
): Promise<{ data: Uint8Array; widthPx: number; heightPx: number } | null> {
  if (typeof document === 'undefined') return null;

  // 仮canvasでテキスト幅を測定
  const measureCanvas = document.createElement('canvas');
  measureCanvas.width = 1;
  measureCanvas.height = 1;
  const measureCtx = measureCanvas.getContext('2d');
  if (!measureCtx) return null;

  const fontSizePx = fontSizePt * 4; // 高解像度化
  measureCtx.font = `${fontSizePx}px sans-serif`;
  const metrics = measureCtx.measureText(text);
  const width = Math.ceil(metrics.width) + 4;
  const height = Math.ceil(fontSizePx * 1.4);

  // 本描画
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;

  // 透明背景ではなく白背景（PDF埋め込み時に背景が見えないように）
  ctx.fillStyle = '#FFFFFF';
  ctx.fillRect(0, 0, width, height);
  ctx.fillStyle = '#666666'; // グレーで控えめに
  ctx.font = `${fontSizePx}px sans-serif`;
  ctx.textAlign = 'left';
  ctx.textBaseline = 'middle';
  ctx.fillText(text, 2, height / 2);

  const dataUrl = canvas.toDataURL('image/png');
  const base64 = dataUrl.split(',')[1];
  const data = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
  return { data, widthPx: width, heightPx: height };
}

// ドット絵風の星マーカー（十字型、バウンディングボックス: x〜x+size, y-size〜y）
function drawStarMarker(
  page: ReturnType<PDFDocument['addPage']>,
  x: number,
  y: number,
  size: number,
  filled: boolean,
) {
  const unit = size / 5;
  const c = rgb(0, 0, 0);

  // 十字をバウンディングボックスの中央に配置
  // 縦棒: x方向は中央3unit、y方向は全高(5unit)
  // 横棒: x方向は全幅(5unit)、y方向は中央3unit
  if (filled) {
    page.drawRectangle({
      x: x + unit,
      y: y - size,
      width: unit * 3,
      height: size,
      color: c,
    });
    page.drawRectangle({
      x,
      y: y - unit * 4,
      width: size,
      height: unit * 3,
      color: c,
    });
  } else {
    page.drawRectangle({
      x: x + unit,
      y: y - size,
      width: unit * 3,
      height: size,
      borderColor: c,
      borderWidth: 0.5,
    });
    page.drawRectangle({
      x,
      y: y - unit * 4,
      width: size,
      height: unit * 3,
      borderColor: c,
      borderWidth: 0.5,
    });
  }
}
