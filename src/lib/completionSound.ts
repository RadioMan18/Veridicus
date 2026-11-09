/**
 * Optional completion chime for long-running commands.
 *
 * Tones are synthesised with Web Audio rather than bundled as files: the app CSP
 * is `default-src 'self'` with no `media-src`, so an `<audio>` asset would be
 * blocked, while an oscillator is generated in memory and needs no permission.
 *
 * The policy lives in `shouldChime` so it can be reasoned about without an audio
 * device, and every entry point swallows its own errors: a missing output device
 * must never fail a run.
 */

import {
  MIN_NOTIFY_RUN_MS,
  notificationText,
  readNotifyEnabled,
  sendDesktopNotification,
  shouldNotify,
} from './desktopNotify';

export type ChimeMode = 'background' | 'always' | 'off';
export type ChimeKind = 'success' | 'failure';

const STORAGE_KEY = 'careeros:completion-sound';
const DEFAULT_MODE: ChimeMode = 'background';
/** Runs shorter than this are silent; the sound is for runs that outlived your attention. */
export const MIN_CHIME_RUN_MS = MIN_NOTIFY_RUN_MS;
/** Two completions in a burst (a compound command, a retry) should not overlap. */
const BURST_GUARD_MS = 400;

export function readChimeMode(): ChimeMode {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === 'always' || stored === 'off' || stored === 'background') {
      return stored;
    }
  } catch {
    // Storage is a convenience; fall back to the default.
  }
  return DEFAULT_MODE;
}

export function writeChimeMode(mode: ChimeMode) {
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    // A preference that cannot be saved should not break the settings screen.
  }
}

/** True when the window is minimised, behind another window, or on another desktop. */
export function isBackgrounded(): boolean {
  if (typeof document === 'undefined') return false;
  return document.hidden || !document.hasFocus();
}

export function shouldChime(options: {
  mode: ChimeMode;
  backgrounded: boolean;
  durationMs: number;
  /** A cancelled run is the user's own doing; it stays quiet. */
  cancelled?: boolean;
  /** Sessions that are long by design (a mock interview) pass 0. */
  minDurationMs?: number;
}): boolean {
  if (options.cancelled) return false;
  if (options.mode === 'off') return false;
  if (options.mode === 'background' && !options.backgrounded) return false;
  return options.durationMs >= (options.minDurationMs ?? MIN_CHIME_RUN_MS);
}

let context: AudioContext | null = null;
let lastPlayedAt = 0;

function audioContext(): AudioContext | null {
  if (typeof window === 'undefined') return null;
  const Constructor: typeof AudioContext | undefined =
    window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!Constructor) return null;
  if (!context) {
    try {
      context = new Constructor();
    } catch {
      return null;
    }
  }
  return context;
}

/**
 * Unlock the audio context on the first user gesture. Browsers start it suspended
 * until then, which would silently swallow a chime played six minutes later.
 */
export function primeCompletionAudio() {
  const ctx = audioContext();
  if (!ctx) return;
  if (ctx.state === 'suspended') {
    void ctx.resume().catch(() => {});
  }
}

function tone(ctx: AudioContext, frequency: number, startsAt: number, durationSeconds: number) {
  const oscillator = ctx.createOscillator();
  const gain = ctx.createGain();
  oscillator.type = 'sine';
  oscillator.frequency.value = frequency;
  gain.gain.setValueAtTime(0.0001, startsAt);
  gain.gain.linearRampToValueAtTime(0.06, startsAt + 0.012);
  gain.gain.exponentialRampToValueAtTime(0.0001, startsAt + durationSeconds);
  oscillator.connect(gain);
  gain.connect(ctx.destination);
  oscillator.start(startsAt);
  oscillator.stop(startsAt + durationSeconds + 0.02);
}

export function playChime(kind: ChimeKind) {
  const now = Date.now();
  if (now - lastPlayedAt < BURST_GUARD_MS) return;
  lastPlayedAt = now;

  const ctx = audioContext();
  if (!ctx) return;
  if (ctx.state === 'suspended') {
    void ctx.resume().catch(() => {});
  }
  try {
    const start = ctx.currentTime + 0.02;
    if (kind === 'failure') {
      tone(ctx, 330, start, 0.3);
    } else {
      tone(ctx, 660, start, 0.14);
      tone(ctx, 880, start + 0.15, 0.18);
    }
  } catch {
    // Audio is a courtesy; never let it break the caller.
  }
}

function notify(
  kind: ChimeKind,
  durationMs: number,
  minDurationMs: number | undefined,
  cancelled: boolean,
  label: string | undefined,
) {
  let backgrounded = false;
  try {
    backgrounded = isBackgrounded();
    if (
      shouldChime({
        mode: readChimeMode(),
        backgrounded,
        durationMs,
        cancelled,
        minDurationMs,
      })
    ) {
      playChime(kind);
    }
  } catch {
    // A missing audio device must not stop the toast below.
  }

  try {
    if (
      !shouldNotify({
        enabled: readNotifyEnabled(),
        backgrounded,
        durationMs,
        cancelled,
        minDurationMs,
      })
    ) {
      return;
    }
    void sendDesktopNotification(notificationText({ kind, label }));
  } catch {
    // Same rule as above.
  }
}

/**
 * Run one user-visible operation and signal when it finishes, if the preference
 * and the timing allow. Wrap the whole operation, never its individual invokes,
 * so a compound command produces one sound and one toast.
 */
export async function withCompletionChime<T>(
  run: () => Promise<T>,
  options: {
    /** Outcomes that count as a failure even though the call resolved. */
    failed?: (value: T) => boolean;
    /** Outcomes the user caused themselves (a cancel) stay silent. */
    cancelled?: (value: T) => boolean;
    /** Override the eight-second floor; a mock interview session passes 0. */
    minDurationMs?: number;
    /** Toast body, e.g. "Cover letter". Keep it generic: no employer, role or document text. */
    label?: string;
  } = {},
): Promise<T> {
  const startedAt = Date.now();
  try {
    const value = await run();
    const elapsed = Date.now() - startedAt;
    notify(
      options.failed?.(value) ? 'failure' : 'success',
      elapsed,
      options.minDurationMs,
      options.cancelled?.(value) ?? false,
      options.label,
    );
    return value;
  } catch (error) {
    notify('failure', Date.now() - startedAt, options.minDurationMs, false, options.label);
    throw error;
  }
}
