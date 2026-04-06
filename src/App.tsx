import { createSignal, Show } from 'solid-js';
import Header from './components/Header';
import Footer from './components/Footer';
import Home from './pages/Home';
import Template from './pages/Template';
import Upload from './pages/Upload';
import About from './pages/About';

export type Page = 'home' | 'template' | 'upload' | 'about';

export default function App() {
  const [page, setPage] = createSignal<Page>('home');
  const [fontName, setFontName] = createSignal('MyHandwriting');

  return (
    <div class="app">
      <Header currentPage={page()} onNavigate={setPage} />
      <main class="main">
        <Show when={page() === 'home'}>
          <Home onNavigate={setPage} />
        </Show>
        <Show when={page() === 'template'}>
          <Template fontName={fontName()} onFontNameChange={setFontName} />
        </Show>
        <Show when={page() === 'upload'}>
          <Upload fontName={fontName()} />
        </Show>
        <Show when={page() === 'about'}>
          <About />
        </Show>
      </main>
      <Footer onNavigate={setPage} />
    </div>
  );
}
