import type { ITheme } from '@xterm/xterm';

function readCssToken(styles: CSSStyleDeclaration, name: string): string | undefined {
  const value = styles.getPropertyValue(name).trim();
  return value.length > 0 ? value : undefined;
}

export function buildTerminalTheme(): ITheme {
  if (typeof document === 'undefined' || typeof getComputedStyle !== 'function') {
    return {};
  }

  const styles = getComputedStyle(document.documentElement);
  const bgBase = readCssToken(styles, '--qn-bg-base');
  const textPrimary = readCssToken(styles, '--qn-text-primary');
  const textSecondary = readCssToken(styles, '--qn-text-secondary');
  const textMuted = readCssToken(styles, '--qn-text-muted');
  const bgActive = readCssToken(styles, '--qn-bg-active');
  const accent = readCssToken(styles, '--qn-accent');
  const danger = readCssToken(styles, '--qn-danger');
  const success = readCssToken(styles, '--qn-success');
  const warning = readCssToken(styles, '--qn-warning');

  const theme: ITheme = {};
  if (bgBase) {
    theme.background = bgBase;
    theme.cursorAccent = bgBase;
  }
  if (textPrimary) {
    theme.foreground = textPrimary;
    theme.cursor = textPrimary;
  }
  if (bgActive) {
    theme.selectionBackground = bgActive;
  }
  if (textMuted) {
    theme.brightBlack = textMuted;
  }
  if (textSecondary) {
    theme.white = textSecondary;
  }
  if (accent) {
    theme.blue = accent;
    theme.brightBlue = accent;
  }
  if (danger) {
    theme.red = danger;
    theme.brightRed = danger;
  }
  if (success) {
    theme.green = success;
    theme.brightGreen = success;
  }
  if (warning) {
    theme.yellow = warning;
    theme.brightYellow = warning;
  }

  return theme;
}

export function observeTerminalThemeChange(onChange: (theme: ITheme) => void): () => void {
  if (typeof document === 'undefined' || typeof window === 'undefined') {
    return () => {};
  }

  const notify = () => {
    onChange(buildTerminalTheme());
  };

  let observer: MutationObserver | null = null;
  if (typeof MutationObserver !== 'undefined') {
    observer = new MutationObserver(() => {
      notify();
    });
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-theme'],
    });
  }

  let mediaQuery: MediaQueryList | null = null;
  const onMediaChange = () => {
    notify();
  };
  if (typeof window.matchMedia === 'function') {
    mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    if (typeof mediaQuery.addEventListener === 'function') {
      mediaQuery.addEventListener('change', onMediaChange);
    }
  }

  return () => {
    observer?.disconnect();
    observer = null;
    if (mediaQuery && typeof mediaQuery.removeEventListener === 'function') {
      mediaQuery.removeEventListener('change', onMediaChange);
    }
  };
}

const utf8Encoder = new TextEncoder();

export function encodeUtf8ToBase64(text: string): string {
  const bytes = utf8Encoder.encode(text);
  let binary = '';
  const chunkSize = 0x8000;
  for (let i = 0; i < bytes.length; i += chunkSize) {
    const slice = bytes.subarray(i, i + chunkSize);
    binary += String.fromCharCode(...slice);
  }
  return btoa(binary);
}

export function decodeBase64ToBytes(dataBase64: string): Uint8Array {
  const binary = atob(dataBase64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

export interface StreamingBase64Decoder {
  decode(dataBase64: string): string;
  flush(): string;
  reset(): void;
}

export function createStreamingBase64Decoder(): StreamingBase64Decoder {
  let decoder = new TextDecoder('utf-8', { fatal: false });
  return {
    decode(dataBase64: string): string {
      if (!dataBase64) {
        return '';
      }
      const bytes = decodeBase64ToBytes(dataBase64);
      return decoder.decode(bytes, { stream: true });
    },
    flush(): string {
      return decoder.decode(new Uint8Array(0), { stream: false });
    },
    reset(): void {
      decoder = new TextDecoder('utf-8', { fatal: false });
    },
  };
}
