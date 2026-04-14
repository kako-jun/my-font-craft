import { defineConfig } from 'vite';
import solidPlugin from 'vite-plugin-solid';

export default defineConfig({
  plugins: [solidPlugin()],
  build: {
    target: 'esnext',
  },
  // WASM ファイルを正しくサーブするための設定
  optimizeDeps: {
    exclude: ['mfc'],
  },
  assetsInclude: ['**/*.wasm'],
});
