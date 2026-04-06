import { createSignal } from 'solid-js';
import { TOTAL_PAGES } from '../data/characters';
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

  async function handleDownload() {
    setGenerating(true);
    try {
      const pdfBytes = await generateTemplatePDF({
        fontName: props.fontName,
        includeHiragana: includeHiragana(),
        includeKatakana: includeKatakana(),
        includeKanji: includeKanji(),
        includeAlphaNum: includeAlphaNum(),
      });
      const blob = new Blob([pdfBytes], { type: 'application/pdf' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'MyFontCraft-template.pdf';
      a.click();
      URL.revokeObjectURL(url);
    } finally {
      setGenerating(false);
    }
  }

  return (
    <div class="template-page">
      <h2>テンプレート生成</h2>

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
              カタカナ（86文字）
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
          約{TOTAL_PAGES}ページのPDFが生成されます。
        </p>

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
