import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const pageFiles = [
  'src/pages/Home.tsx',
  'src/pages/Template.tsx',
  'src/pages/Upload.tsx',
  'src/pages/About.tsx',
  'src/pages/NotFound.tsx',
];

function read(path: string): string {
  return readFileSync(path, 'utf8');
}

describe('UI doctrine', () => {
  it('does not use page-specific structure classes for informational pages', () => {
    const forbidden = [
      'home',
      'template-page',
      'upload-page',
      'about-page',
      'not-found',
      'drop-zone',
      'shooting-guide',
      'upload-done',
      'about-author',
      'about-links',
    ];
    const sources = pageFiles.map((file) => `${file}\n${read(file)}`).join('\n');

    for (const className of forbidden) {
      expect(sources).not.toContain(`class="${className}`);
      expect(sources).not.toContain(`'${className}`);
    }
  });

  it('uses one action style for normal links and buttons', () => {
    const sources = pageFiles.map(read).join('\n');

    expect(sources).not.toContain('act--primary');
    expect(sources).not.toContain('act--quiet');
  });

  it('keeps file selection wording consistent', () => {
    const upload = read('src/pages/Upload.tsx');

    expect(upload).not.toContain('画像を選ぶ');
    expect(upload).toContain('撮影画像を選択');
    expect(upload).toContain('既存フォントを選択');
    expect(upload).toContain('フォルダを選択');
    expect(upload).toContain('ZIPを選択');
  });

  it('keeps page spacing in the shared section system', () => {
    const css = read('src/styles/global.css');

    expect(css).not.toMatch(/h1\s*\{[^}]*margin-bottom/s);
    expect(css).not.toMatch(/h2\s*\{[^}]*margin-top/s);
    expect(css).toMatch(/\.page-section\s*\{[^}]*margin-top:\s*var\(--section-gap\)/s);
  });

  it('does not keep obsolete page-specific CSS selectors', () => {
    const css = read('src/styles/global.css');
    const forbiddenSelectors = [
      '.home',
      '.template-page',
      '.upload-page',
      '.about-page',
      '.not-found',
      '.sample-slip',
      '.drop-zone',
      '.shooting-guide',
      '.upload-done',
      '.about-author',
      '.about-links',
    ];

    for (const selector of forbiddenSelectors) {
      expect(css).not.toContain(selector);
    }
  });
});
