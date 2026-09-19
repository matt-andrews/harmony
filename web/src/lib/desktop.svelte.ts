// The bridge to the desktop window. Inside harmony-desktop the webview exposes
// `window.ipc` and the window injects `__HARMONY_DESKTOP__` before any script
// runs; in a plain browser neither exists and everything here is a no-op.

type DesktopMessage =
  | { type: 'compact' }
  | { type: 'expand' }
  | { type: 'drag' }
  | { type: 'set_compact_on_start'; value: boolean };

declare global {
  interface Window {
    ipc?: { postMessage(message: string): void };
    __HARMONY_DESKTOP__?: { compactOnStart?: boolean };
  }
}

export const desktop = $state({
  available: !!window.ipc && !!window.__HARMONY_DESKTOP__,
  /** The window is the floating always-on-top strip rather than the full app. */
  compact: false,
  /** Collapse into the strip whenever a session starts. */
  compactOnStart: window.__HARMONY_DESKTOP__?.compactOnStart ?? false,
});

function post(message: DesktopMessage): void {
  window.ipc?.postMessage(JSON.stringify(message));
}

function setCompact(compact: boolean): void {
  if (!desktop.available || desktop.compact === compact) return;
  desktop.compact = compact;
  document.documentElement.classList.toggle('compact', compact);
  post({ type: compact ? 'compact' : 'expand' });
}

export function enterCompact(): void {
  setCompact(true);
}

export function exitCompact(): void {
  setCompact(false);
}

/** Hand the pointer to the OS so the borderless strip can be dragged around. */
export function startDrag(): void {
  if (desktop.compact) post({ type: 'drag' });
}

export function setCompactOnStart(value: boolean): void {
  if (!desktop.available) return;
  desktop.compactOnStart = value;
  post({ type: 'set_compact_on_start', value });
}
