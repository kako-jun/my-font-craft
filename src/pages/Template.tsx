import { createSignal, createMemo, Show } from 'solid-js';
import { A } from '@solidjs/router';
import {
  HIRAGANA,
  KATAKANA,
  UPPERCASE,
  LOWERCASE,
  DIGITS,
  ASCII_SYMBOLS,
  JP_SYMBOLS,
  CHARS_PER_PAGE,
} from '../data/characters';
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

  const noneSelected = createMemo(
    () => !includeHiragana() && !includeKatakana() && !includeAlphaNum() && !includeKanji(),
  );

  const estimatedPages = createMemo(() => {
    let count = 0;
    if (includeHiragana()) count += HIRAGANA.length;
    if (includeKatakana()) count += KATAKANA.length;
    if (includeKanji()) count += JOYO_KANJI.length;
    if (includeAlphaNum())
      count +=
        UPPERCASE.length +
        LOWERCASE.length +
        DIGITS.length +
        ASCII_SYMBOLS.length +
        JP_SYMBOLS.length;
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
      const blob = new Blob([new Uint8Array(pdfBytes)], { type: 'application/pdf' });
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
    <div>
      <h1>テンプレートを印刷する</h1>

      <section class="page-section">
        <h2>フォント名を入力してください</h2>
        <div class="section__body">
          <p>任意です。空欄でも作成でき、あとで変更できます。</p>
          <input
            id="font-name"
            aria-label="フォント名"
            class="input"
            type="text"
            value={props.fontName}
            onInput={(e) => props.onFontNameChange(e.currentTarget.value)}
            placeholder="フォント名"
          />
        </div>
      </section>

      <section class="page-section">
        <h2>対象文字を選んでください</h2>
        <div class="section__body">
          <div class="checkbox-group">
            <label>
              <input
                type="checkbox"
                checked={includeHiragana()}
                onChange={(e) => setIncludeHiragana(e.currentTarget.checked)}
              />
              ひらがな（83文字）
            </label>
            <label>
              <input
                type="checkbox"
                checked={includeKatakana()}
                onChange={(e) => setIncludeKatakana(e.currentTarget.checked)}
              />
              カタカナ（87文字）
            </label>
            <label>
              <input
                type="checkbox"
                checked={includeAlphaNum()}
                onChange={(e) => setIncludeAlphaNum(e.currentTarget.checked)}
              />
              英数字・記号（102文字）
            </label>
            <label>
              <input
                type="checkbox"
                checked={includeKanji()}
                onChange={(e) => setIncludeKanji(e.currentTarget.checked)}
              />
              常用漢字（2,136文字）
            </label>
          </div>
        </div>
      </section>

      <section class="page-section">
        <h2>印刷内容を確認してください</h2>
        <div class="section__body">
          <p>
            PDF 約<span class="num">{estimatedPages()}</span>ページです。
          </p>
          <p>青い内枠とガイド線を目安に一文字ずつ書いてください。</p>
          <p>チェック欄は任意です。</p>
          <p>同じ文字を2マス書いたときはチェックした方を優先します。</p>
        </div>
      </section>

      <section class="page-section">
        <h2>テンプレートを印刷する</h2>
        <div class="section__body">
          <p>PDFをダウンロードして印刷します。</p>
          {error() && <div class="message message--error">{error()}</div>}

          <Show when={noneSelected()}>
            <p class="message message--warning">1つ以上選択してください</p>
          </Show>

          <button class="act" onClick={handleDownload} disabled={generating() || noneSelected()}>
            {generating() ? 'PDF生成中' : 'PDFをダウンロード'}
          </button>
        </div>
      </section>

      <section class="page-section">
        <h2>記入後に撮影へ進んでください</h2>
        <div class="section__body">
          <p>
            印刷と記入が終わったら、
            <A class="act" href="/upload">
              撮影画像からフォントを作成する
            </A>
            へ進みます。
          </p>
        </div>
      </section>
    </div>
  );
}
