import { describe, expect, it } from 'vitest';
import { buildTemplateTitle, TEMPLATE_WRITE_GUIDE } from '../../src/lib/template/generator';

describe('template PDF header and guide text', () => {
  it('does not show a default-looking font name on first print', () => {
    expect(buildTemplateTitle('')).toBe('MyFontCraft');
    expect(buildTemplateTitle('   ')).toBe('MyFontCraft');
  });

  it('shows a font name only when the user explicitly entered one', () => {
    expect(buildTemplateTitle('KakoHand')).toBe('MyFontCraft — "KakoHand"');
    expect(buildTemplateTitle('  KakoHand  ')).toBe('MyFontCraft — "KakoHand"');
  });

  it('names the blue inner frame and guide lines in the printed footer', () => {
    expect(TEMPLATE_WRITE_GUIDE).toContain('青い内枠');
    expect(TEMPLATE_WRITE_GUIDE).toContain('ガイド線');
    expect(TEMPLATE_WRITE_GUIDE).toContain('チェック欄');
  });
});
