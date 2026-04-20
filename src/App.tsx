import { createSignal, type JSX } from 'solid-js';
import { Router, Route } from '@solidjs/router';
import Header from './components/Header';
import Footer from './components/Footer';
import Home from './pages/Home';
import Template from './pages/Template';
import Upload from './pages/Upload';
import About from './pages/About';

export default function App() {
  const [fontName, setFontName] = createSignal('MyHandwriting');

  const Layout = (props: { children?: JSX.Element }) => (
    <div class="app">
      <Header />
      <main class="main">{props.children}</main>
      <Footer />
    </div>
  );

  return (
    <Router root={Layout}>
      <Route path="/" component={Home} />
      <Route
        path="/template"
        component={() => <Template fontName={fontName()} onFontNameChange={setFontName} />}
      />
      <Route path="/upload" component={() => <Upload fontName={fontName()} />} />
      <Route path="/about" component={About} />
      <Route path="*" component={Home} />
    </Router>
  );
}
