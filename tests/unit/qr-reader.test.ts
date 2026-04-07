/**
 * QRコード読み取りのユニットテスト
 * 模擬スキャン画像からQRコードを読み取り、ページ番号等が正しいか検証
 */
import { describe, it, expect, beforeAll } from 'vitest';
import '../helpers/canvas-polyfill';
import { createCanvas, loadImage } from 'canvas';
import * as path from 'node:path';
import { readQRFromImageData } from '../../src/lib/scanner/qr-reader';

const MOCK_DIR = path.join(import.meta.dirname, '..', 'fixtures', 'mock-scans');

describe('QR Reader', () => {
  it('should read QR from page 1', async () => {
    const img = await loadImage(path.join(MOCK_DIR, 'page-01.png'));
    const canvas = createCanvas(img.width, img.height);
    const ctx = canvas.getContext('2d');
    ctx.drawImage(img, 0, 0);

    // ヘッダー領域（上部20%）を切り出して読み取り
    const headerH = Math.floor(img.height * 0.2);
    const headerData = ctx.getImageData(0, 0, img.width, headerH);
    const qr = readQRFromImageData(headerData as any);

    expect(qr).not.toBeNull();
    expect(qr!.p).toBe('mfc');
    expect(qr!.v).toBe(1);
    expect(qr!.pg).toBe(1);
    expect(qr!.t).toBe(2); // ひらがな83文字 ÷ 48 = 2ページ
    expect(qr!.m).toBe(2);
  });

  it('should read QR from page 2', async () => {
    const img = await loadImage(path.join(MOCK_DIR, 'page-02.png'));
    const canvas = createCanvas(img.width, img.height);
    const ctx = canvas.getContext('2d');
    ctx.drawImage(img, 0, 0);

    const headerH = Math.floor(img.height * 0.2);
    const headerData = ctx.getImageData(0, 0, img.width, headerH);
    const qr = readQRFromImageData(headerData as any);

    expect(qr).not.toBeNull();
    expect(qr!.pg).toBe(2);
    expect(qr!.t).toBe(2);
  });

  it('should read QR from full image when header crop misses', async () => {
    const img = await loadImage(path.join(MOCK_DIR, 'page-02.png'));
    const canvas = createCanvas(img.width, img.height);
    const ctx = canvas.getContext('2d');
    ctx.drawImage(img, 0, 0);

    // 全体から探索
    const fullData = ctx.getImageData(0, 0, img.width, img.height);
    const qr = readQRFromImageData(fullData as any);

    expect(qr).not.toBeNull();
    expect(qr!.pg).toBe(2);
  });
});
