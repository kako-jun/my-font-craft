import { describe, it, expect, vi } from 'vitest';
import {
  INSTALL_DISMISSED_KEY,
  isInstallDismissed,
  isStandaloneDisplay,
  markInstallDismissed,
  shouldShowInstallPrompt,
  type StorageLike,
} from '../../src/lib/pwaInstall';

/** StorageLike のインメモリ実装（localStorage 実体なしでテストするため） */
function makeMemoryStorage(initial: Record<string, string> = {}): StorageLike {
  const store = new Map(Object.entries(initial));
  return {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => {
      store.set(key, value);
    },
  };
}

describe('isInstallDismissed', () => {
  it('キーが未設定なら false', () => {
    expect(isInstallDismissed(makeMemoryStorage())).toBe(false);
  });

  it('却下記憶キーが "1" なら true', () => {
    const storage = makeMemoryStorage({ [INSTALL_DISMISSED_KEY]: '1' });
    expect(isInstallDismissed(storage)).toBe(true);
  });

  it('無関係な値が入っていても true にはならない', () => {
    const storage = makeMemoryStorage({ [INSTALL_DISMISSED_KEY]: 'yes' });
    expect(isInstallDismissed(storage)).toBe(false);
  });

  it('Storage.getItem が例外を投げても false にフォールバックする（プライベートモード等）', () => {
    const storage: StorageLike = {
      getItem: () => {
        throw new Error('blocked');
      },
      setItem: vi.fn(),
    };
    expect(isInstallDismissed(storage)).toBe(false);
  });
});

describe('markInstallDismissed', () => {
  it('却下キーを "1" で書き込む', () => {
    const storage = makeMemoryStorage();
    markInstallDismissed(storage);
    expect(isInstallDismissed(storage)).toBe(true);
  });

  it('Storage.setItem が例外を投げても握りつぶす', () => {
    const storage: StorageLike = {
      getItem: () => null,
      setItem: () => {
        throw new Error('blocked');
      },
    };
    expect(() => markInstallDismissed(storage)).not.toThrow();
  });
});

describe('isStandaloneDisplay', () => {
  it('display-mode: standalone にマッチしたら true', () => {
    expect(isStandaloneDisplay(true, undefined)).toBe(true);
  });

  it('iOS の navigator.standalone が true でも true', () => {
    expect(isStandaloneDisplay(false, true)).toBe(true);
  });

  it('どちらも該当しなければ false', () => {
    expect(isStandaloneDisplay(false, undefined)).toBe(false);
    expect(isStandaloneDisplay(false, false)).toBe(false);
  });
});

describe('shouldShowInstallPrompt', () => {
  it('イベント取得済み・未却下・未インストールなら表示する', () => {
    expect(shouldShowInstallPrompt({ hasPrompt: true, dismissed: false, installed: false })).toBe(
      true,
    );
  });

  it('beforeinstallprompt 未発火なら表示しない', () => {
    expect(shouldShowInstallPrompt({ hasPrompt: false, dismissed: false, installed: false })).toBe(
      false,
    );
  });

  it('却下済みなら表示しない', () => {
    expect(shouldShowInstallPrompt({ hasPrompt: true, dismissed: true, installed: false })).toBe(
      false,
    );
  });

  it('インストール済みなら表示しない', () => {
    expect(shouldShowInstallPrompt({ hasPrompt: true, dismissed: false, installed: true })).toBe(
      false,
    );
  });
});
