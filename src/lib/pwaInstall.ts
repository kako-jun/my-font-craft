/**
 * PWA インストール促し（#124）: beforeinstallprompt のキャッチと却下記憶。
 *
 * mypace (apps/web/src/hooks/ui/usePWAInstall.ts) の移植。ただしこのリポは Solid.js
 * のため hooks 化はせず、DOM に依存しない判定ロジックだけを純粋関数として切り出す
 * （テスト対象）。DOM イベント配線・レンダリングは components/InstallPrompt.tsx が担う。
 *
 * mypace との差分（意図的な設計判断）: mypace は却下から7日経過すると再表示するが
 * （DISMISS_DURATION_MS）、本実装は Issue #124 の要件「一度却下したら再表示しない」に
 * 合わせて恒久非表示にしている。7日タイマーは移し忘れではなく採用していない。
 */

/** Chrome/Edge が発火する beforeinstallprompt イベントの型（標準 lib.dom.d.ts に未収録） */
export interface BeforeInstallPromptEvent extends Event {
  readonly platforms: string[];
  readonly userChoice: Promise<{ outcome: 'accepted' | 'dismissed'; platform: string }>;
  prompt(): Promise<void>;
}

export const INSTALL_DISMISSED_KEY = 'mfc-pwa-install-dismissed';

/** localStorage 相当の最小インターフェース（テストで差し替え可能にするため） */
export type StorageLike = Pick<Storage, 'getItem' | 'setItem'>;

/**
 * 却下済みかどうかを判定する。
 * Storage 読み取り失敗（プライベートモード等）は「未却下」（＝表示継続）にフォールバックする。
 */
export function isInstallDismissed(storage: StorageLike): boolean {
  try {
    return storage.getItem(INSTALL_DISMISSED_KEY) === '1';
  } catch {
    return false;
  }
}

/** 却下を記憶する。書き込み失敗は握りつぶす（バナーが閉じるだけで実害はない）。 */
export function markInstallDismissed(storage: StorageLike): void {
  try {
    storage.setItem(INSTALL_DISMISSED_KEY, '1');
  } catch {
    /* noop */
  }
}

/** 既にインストール済み（standalone 表示）かどうかを判定する。 */
export function isStandaloneDisplay(
  mediaMatches: boolean,
  iosStandalone: boolean | undefined,
): boolean {
  return mediaMatches || iosStandalone === true;
}

/** バナーを表示すべきかどうかの最終判定。 */
export function shouldShowInstallPrompt(params: {
  hasPrompt: boolean;
  dismissed: boolean;
  installed: boolean;
}): boolean {
  return params.hasPrompt && !params.dismissed && !params.installed;
}
