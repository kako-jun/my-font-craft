import { createSignal, createMemo } from 'solid-js';
import { HIRAGANA, KATAKANA, UPPERCASE, LOWERCASE, DIGITS, ASCII_SYMBOLS, JP_SYMBOLS, CHARS_PER_PAGE } from '../data/characters';
import { JOYO_KANJI } from '../data/joyo-kanji';
import { generateTemplatePDF } from '../lib/template/generator';

interface Props {
  fontName: string;
  onFontNameChange: (name: string) => void;
}

export default function Template(props: Props) {
  const [includeHiragana, setIncludeHiragana] = createSignal(true);
  const [includeKatakana, setIncludeKatakana] = createSignal(true);
  const [includeKanji, setIncludeKanji] = createSignal(true);
  const [includeAlphaNum, setIncludeAlphaNum] = createSignal(true);
  const [generating, setGenerating] = createSignal(false);
  const [error, setError] = createSignal('');

  const estimatedPages = createMemo(() => {
    let count = 0;
    if (includeHiragana()) count += HIRAGANA.length;
    if (includeKatakana()) count += KATAKANA.length;
    if (includeKanji()) count += JOYO_KANJI.length;
    if (includeAlphaNum()) count += UPPERCASE.length + LOWERCASE.length + DIGITS.length + ASCII_SYMBOLS.length + JP_SYMBOLS.length;
    return Math.ceil(count / CHARS_PER_PAGE);
  });

  async function handleDownload() {
    setGenerating(true);
    setError('');
    try {
      const pdfBytes = await generateTemplatePDF({
        fontName: props.fontName,
        includeHiragana: includeHiragana(),
        includeKatakana: includeKatakana(),
        includeKanji: includeKanji(),
        includeAlphaNum: includeAlphaNum(),
      });
      const blob = new Blob([pdfBytes.buffer], { type: 'application/pdf' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'MyFontCraft-template.pdf';
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      setError(`PDF生成に失敗しました: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setGenerating(false);
    }
  }

  return (
    <div class="template-page">
      <h2>1. テンプレートを印刷する</h2>

      <div class="card">
        <div class="form-group">
          <label for="font-name">フォント名</label>
          <input
            id="font-name"
            class="input"
            type="text"
            value={props.fontName}
            onInput={(e) => props.onFontNameChange(e.currentTarget.value)}
            placeholder="MyHandwriting"
          />
        </div>

        <div class="form-group">
          <label>対象文字</label>
          <div class="checkbox-group">
            <label>
              <input type="checkbox" checked={includeHiragana()} onChange={(e) => setIncludeHiragana(e.currentTarget.checked)} />
              ひらがな（83文字）
            </label>
            <label>
              <input type="checkbox" checked={includeKatakana()} onChange={(e) => setIncludeKatakana(e.currentTarget.checked)} />
              カタカナ（87文字）
            </label>
            <label>
              <input type="checkbox" checked={includeKanji()} onChange={(e) => setIncludeKanji(e.currentTarget.checked)} />
              常用漢字（2,136文字）
            </label>
            <label>
              <input type="checkbox" checked={includeAlphaNum()} onChange={(e) => setIncludeAlphaNum(e.currentTarget.checked)} />
              英数字・記号（102文字）
            </label>
          </div>
        </div>

        <p class="template-page__info">
          約{estimatedPages()}ページのPDFが生成されます。
        </p>

        {error() && <div class="message message--error">{error()}</div>}

        <button
          class="btn btn--primary"
          onClick={handleDownload}
          disabled={generating()}
        >
          {generating() ? 'PDF生成中...' : 'PDFをダウンロード'}
        </button>
      </div>
    </div>
  );
}
