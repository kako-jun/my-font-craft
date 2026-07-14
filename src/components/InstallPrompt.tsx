import { createSignal, onCleanup, onMount, Show } from 'solid-js';
import {
  type BeforeInstallPromptEvent,
  isInstallDismissed,
  isStandaloneDisplay,
  markInstallDismissed,
  shouldShowInstallPrompt,
} from '../lib/pwaInstall';

/**
 * PWA インストール促しバナー（#124）。
 * beforeinstallprompt を捕まえて画面上部に表示する。既にインストール済み
 * （standalone 表示）なら出さない。却下したら localStorage に記憶し、以後出さない。
 */
export default function InstallPrompt() {
  const [deferredPrompt, setDeferredPrompt] = createSignal<BeforeInstallPromptEvent | null>(null);
  const [installed, setInstalled] = createSignal(false);
  const [dismissed, setDismissed] = createSignal(isInstallDismissed(localStorage));

  onMount(() => {
    const standalone = isStandaloneDisplay(
      window.matchMedia('(display-mode: standalone)').matches,
      (window.navigator as Navigator & { standalone?: boolean }).standalone,
    );
    if (standalone) {
      setInstalled(true);
      return;
    }

    const handleBeforeInstallPrompt = (e: Event) => {
      e.preventDefault();
      setDeferredPrompt(e as BeforeInstallPromptEvent);
    };
    const handleAppInstalled = () => {
      setInstalled(true);
      setDeferredPrompt(null);
    };

    window.addEventListener('beforeinstallprompt', handleBeforeInstallPrompt);
    window.addEventListener('appinstalled', handleAppInstalled);

    onCleanup(() => {
      window.removeEventListener('beforeinstallprompt', handleBeforeInstallPrompt);
      window.removeEventListener('appinstalled', handleAppInstalled);
    });
  });

  const visible = () =>
    shouldShowInstallPrompt({
      hasPrompt: deferredPrompt() !== null,
      dismissed: dismissed(),
      installed: installed(),
    });

  const handleInstall = async () => {
    const ev = deferredPrompt();
    if (!ev) return;
    try {
      await ev.prompt();
      await ev.userChoice;
    } catch {
      /* ユーザー操作外でのキャンセル等は無視 */
    }
    setDeferredPrompt(null);
  };

  const handleDismiss = () => {
    markInstallDismissed(localStorage);
    setDismissed(true);
    setDeferredPrompt(null);
  };

  return (
    <Show when={visible()}>
      <div class="install-bar" role="status">
        <p class="message message--info install-bar__text">
          ホーム画面に追加すると、次からすぐ開ける
        </p>
        <div class="install-bar__actions">
          <button type="button" class="act" onClick={handleInstall}>
            追加する
          </button>
          <button type="button" class="act act--quiet" onClick={handleDismiss}>
            閉じる
          </button>
        </div>
      </div>
    </Show>
  );
}
