/**
 * 四隅マーカー検出のユニットテスト
 */
import { describe, it, expect } from 'vitest';
import '../helpers/canvas-polyfill';
import { createCanvas, loadImage } from 'canvas';
import * as path from 'node:path';
import { detectMarkers, detectOrientation } from '../../src/lib/scanner/marker-detector';

const MOCK_DIR = path.join(import.meta.dirname, '..', 'fixtures', 'mock-scans');

describe('Marker Detector', () => {
  it('should detect four corners from page 1', async () => {
    const img = await loadImage(path.join(MOCK_DIR, 'page-01.png'));
    const canvas = createCanvas(img.width, img.height) as any;
    const ctx = canvas.getContext('2d');
    ctx.drawImage(img, 0, 0);

    const corners = detectMarkers(canvas);
    expect(corners).not.toBeNull();

    // 四隅がそれぞれ画像の正しい象限にあることを確認
    const midX = img.width / 2;
    const midY = img.height / 2;

    expect(corners!.topLeft.x).toBeLessThan(midX);
    expect(corners!.topLeft.y).toBeLessThan(midY);

    expect(corners!.topRight.x).toBeGreaterThan(midX);
    expect(corners!.topRight.y).toBeLessThan(midY);

    expect(corners!.bottomLeft.x).toBeLessThan(midX);
    expect(corners!.bottomLeft.y).toBeGreaterThan(midY);

    expect(corners!.bottomRight.x).toBeGreaterThan(midX);
    expect(corners!.bottomRight.y).toBeGreaterThan(midY);
  });

  it('should detect orientation as 0 (upright)', async () => {
    const img = await loadImage(path.join(MOCK_DIR, 'page-01.png'));
    const canvas = createCanvas(img.width, img.height) as any;
    const ctx = canvas.getContext('2d');
    ctx.drawImage(img, 0, 0);

    const corners = detectMarkers(canvas);
    expect(corners).not.toBeNull();

    const rotation = detectOrientation(corners!, canvas);
    expect(rotation).toBe(0);
  });
});
