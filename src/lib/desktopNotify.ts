/**
 * Native Windows toast for finished runs.
 *
 * The rules mirror the completion chime (`shouldChime`) but stand on their own:
 * a toast while you are looking at the window is noise, so notifications are
 * always background-only, whatever the chime preference says.
 *
 * Two details of the Tauri notification plugin shape this file:
 *
 * - `isPermissionGranted()` dereferences `window.Notification.permission` before
 *   it reaches the plugin, which can throw under WebView2. Every call is wrapped.
 * - On desktop the plugin's `request_permission` always resolves to granted
 *   without prompting; the real switch is the Windows per-app notification
 *   setting. The app's own toggle is the consent step, not the OS.
 *
 * Nothing here may ever fail a run: the toast is a courtesy.
 */

export type NotificationKind = 'success' | 'failure';
export interface NotificationText {
  title: string;
  body: string;
}

const STORAGE_KEY = 'careeros:desktop-notifications';
/** Same eight-second floor as the chime; shorter runs did not outlive your attention. */
export const MIN_NOTIFY_RUN_MS = 8_000;

export function readNotifyEnabled(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) !== 'off';
  } catch {
    return true;
  }
}

export function writeNotifyEnabled(enabled: boolean) {
  try {
    localStorage.setItem(STORAGE_KEY, enabled ? 'on' : 'off');
  } catch {
    // A preference that cannot be saved should not break the settings screen.
  }
}

/** True inside the Tauri shell; a plain Vite preview has no OS toasts. */
export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export function shouldNotify(options: {
  enabled: boolean;
  backgrounded: boolean;
  durationMs: number;
  /** A cancelled run is the user's own doing; it stays quiet. */
  cancelled?: boolean;
  /** Sessions that are long by design (a mock interview) pass 0. */
  minDurationMs?: number;
}): boolean {
  if (!options.enabled) return false;
  if (options.cancelled) return false;
  if (!options.backgrounded) return false;
  return options.durationMs >= (options.minDurationMs ?? MIN_NOTIFY_RUN_MS);
}

/**
 * Toast copy is deliberately generic. Notifications persist in the Action
 * Center, so it must never carry an employer, a role, an error message or any
 * document text — only the name of the operation.
 */
export function notificationText(options: { kind: NotificationKind; label?: string }): NotificationText {
  const label = options.label?.trim() || 'Run';
  return options.kind === 'failure'
    ? { title: 'CareerOS', body: `${label} failed — open CareerOS for details.` }
    : { title: 'CareerOS', body: `${label} finished.` };
}

/**
 * Ask for permission once, from a deliberate user action (turning the toggle on).
 * Never call this from a completion handler: a prompt mid-run is worse than no toast.
 */
export async function ensureNotificationPermission(): Promise<boolean> {
  try {
    const api = await import('@tauri-apps/plugin-notification');
    if (await api.isPermissionGranted()) return true;
    return (await api.requestPermission()) === 'granted';
  } catch {
    // WebView2 can throw a TypeError here before the plugin is reached.
    return false;
  }
}

/** Fire and forget. Resolves false when there is no shell or the send failed. */
export async function sendDesktopNotification(text: NotificationText): Promise<boolean> {
  if (!isTauriRuntime()) return false;
  try {
    const api = await import('@tauri-apps/plugin-notification');
    api.sendNotification({ title: text.title, body: text.body });
    return true;
  } catch {
    return false;
  }
}
