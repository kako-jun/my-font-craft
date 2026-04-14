import { describe, it, expect } from 'vitest';
import {
  COLS,
  ROWS,
  SKIPPED_ROW,
  SKIPPED_COL,
  CENTER_MARKER_X,
  CENTER_MARKER_Y,
  CENTER_MARKER_SIZE,
  isSkippedCell,
  gridToCharIndex,
} from '../../src/lib/template/layout';
import { CHARS_PER_PAGE } from '../../src/data/characters';

describe('layout: center marker constants', () => {
  it('center marker is at (103, 147) with size 6', () => {
    expect(CENTER_MARKER_X).toBe(103);
    expect(CENTER_MARKER_Y).toBe(147);
    expect(CENTER_MARKER_SIZE).toBe(6);
  });

  it('center marker center is at (106, 150)', () => {
    expect(CENTER_MARKER_X + CENTER_MARKER_SIZE / 2).toBe(106);
    expect(CENTER_MARKER_Y + CENTER_MARKER_SIZE / 2).toBe(150);
  });
});

describe('layout: skipped cell', () => {
  it('CHARS_PER_PAGE is 47', () => {
    expect(CHARS_PER_PAGE).toBe(47);
    expect(CHARS_PER_PAGE).toBe(COLS * ROWS - 1);
  });

  it('isSkippedCell returns true only for (SKIPPED_ROW, SKIPPED_COL)', () => {
    expect(isSkippedCell(SKIPPED_ROW, SKIPPED_COL)).toBe(true);
    expect(isSkippedCell(0, 0)).toBe(false);
    expect(isSkippedCell(SKIPPED_ROW, 0)).toBe(false);
    expect(isSkippedCell(0, SKIPPED_COL)).toBe(false);
  });

  it('gridToCharIndex returns null for skipped cell', () => {
    expect(gridToCharIndex(SKIPPED_ROW, SKIPPED_COL)).toBeNull();
  });

  it('gridToCharIndex covers exactly 47 indices', () => {
    const indices: number[] = [];
    for (let row = 0; row < ROWS; row++) {
      for (let col = 0; col < COLS; col++) {
        const idx = gridToCharIndex(row, col);
        if (idx !== null) {
          indices.push(idx);
        }
      }
    }
    expect(indices.length).toBe(CHARS_PER_PAGE);
    // All indices should be unique and in range [0, 46]
    const unique = new Set(indices);
    expect(unique.size).toBe(47);
    expect(Math.min(...indices)).toBe(0);
    expect(Math.max(...indices)).toBe(46);
  });

  it('gridToCharIndex is monotonically increasing', () => {
    let prev = -1;
    for (let row = 0; row < ROWS; row++) {
      for (let col = 0; col < COLS; col++) {
        const idx = gridToCharIndex(row, col);
        if (idx !== null) {
          expect(idx).toBeGreaterThan(prev);
          prev = idx;
        }
      }
    }
  });

  it('indices before skipped cell match linear position', () => {
    // Before the skip, index should equal row * COLS + col
    expect(gridToCharIndex(0, 0)).toBe(0);
    expect(gridToCharIndex(0, 3)).toBe(3);
    expect(gridToCharIndex(6, 1)).toBe(25); // just before skip at (6,2)
  });

  it('indices after skipped cell are shifted by -1', () => {
    // (6, 3) is linear 27, but after skip it should be 26
    expect(gridToCharIndex(6, 3)).toBe(26);
    // Last cell (11, 3) is linear 47, after skip = 46
    expect(gridToCharIndex(11, 3)).toBe(46);
  });
});
