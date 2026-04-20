import { createSignal, type JSX } from 'solid-js';
import { Router, Route } from '@solidjs/router';
import Header from './components/Header';
import Footer from './components/Footer';
import Home from './pages/Home';
import Template from './pages/Template';
import Upload from './pages/Upload';
import About from './pages/About';
import NotFound from './pages/NotFound';

function Layout(props: { children?: JSX.Element }) {
  return (
    <div class="app">
      <Header />
      <main class="main">{props.children}</main>
      <Footer />
    </div>
  );
}

export default function App() {
  const [fontName, setFontName] = createSignal('MyHandwriting');

  return (
    <Router root={Layout}>
      <Route path="/" component={Home} />
      <Route
        path="/template"
        component={() => <Template fontName={fontName()} onFontNameChange={setFontName} />}
      />
      <Route path="/upload" component={() => <Upload fontName={fontName()} />} />
      <Route path="/about" component={About} />
      <Route path="*" component={NotFound} />
    </Router>
  );
}
