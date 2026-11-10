import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  MIN_NOTIFY_RUN_MS,
  notificationText,
  readNotifyEnabled,
  sendDesktopNotification,
  shouldNotify,
  writeNotifyEnabled,
} from './desktopNotify'
import { MIN_CHIME_RUN_MS } from './completionSound'

type Policy = Parameters<typeof shouldNotify>[0]

const LONG_RUN_MS = MIN_NOTIFY_RUN_MS + 1_000

/** Backgrounded, long run with the preference on — the case that should toast. */
function policy(overrides: Partial<Policy>): Policy {
  return { enabled: true, backgrounded: true, durationMs: LONG_RUN_MS, ...overrides }
}

describe('shouldNotify', () => {
  it('stays silent when the preference is off', () => {
    expect(shouldNotify(policy({ enabled: false }))).toBe(false)
    expect(shouldNotify(policy({ enabled: false, durationMs: 600_000 }))).toBe(false)
  })

  it('is background-only even for a long run', () => {
    expect(shouldNotify(policy({ backgrounded: false }))).toBe(false)
    expect(shouldNotify(policy({ backgrounded: false, durationMs: 600_000 }))).toBe(false)
  })

  it('requires the eight-second floor', () => {
    expect(shouldNotify(policy({ durationMs: 0 }))).toBe(false)
    expect(shouldNotify(policy({ durationMs: MIN_NOTIFY_RUN_MS - 1 }))).toBe(false)
    expect(shouldNotify(policy({ durationMs: MIN_NOTIFY_RUN_MS }))).toBe(true)
    expect(shouldNotify(policy({ durationMs: MIN_NOTIFY_RUN_MS + 1 }))).toBe(true)
  })

  it('never announces a cancelled run', () => {
    expect(shouldNotify(policy({ cancelled: true }))).toBe(false)
    expect(shouldNotify(policy({ durationMs: 600_000, cancelled: true }))).toBe(false)
  })

  it('accepts a per-call floor for sessions that are long by design', () => {
    expect(shouldNotify(policy({ minDurationMs: 0, durationMs: 0 }))).toBe(true)
    // The zero floor removes the timing test, not the background test.
    expect(shouldNotify(policy({ backgrounded: false, minDurationMs: 0, durationMs: 0 }))).toBe(false)
  })

  it('shares the chime floor so the two cues cannot drift apart', () => {
    expect(MIN_NOTIFY_RUN_MS).toBe(MIN_CHIME_RUN_MS)
  })
})

describe('notificationText', () => {
  it('names the operation, not the contents', () => {
    expect(notificationText({ kind: 'success', label: 'Cover letter' })).toEqual({
      title: 'Veridicus',
      body: 'Cover letter finished.',
    })
  })

  it('points at the app for failures instead of carrying the error', () => {
    expect(notificationText({ kind: 'failure', label: 'Apply workflow' })).toEqual({
      title: 'Veridicus',
      body: 'Apply workflow failed — open Veridicus for details.',
    })
  })

  it('falls back to a neutral label', () => {
    expect(notificationText({ kind: 'success' }).body).toBe('Run finished.')
    expect(notificationText({ kind: 'success', label: '   ' }).body).toBe('Run finished.')
  })
})

function memoryStorage(initial: Record<string, string> = {}) {
  const entries = new Map(Object.entries(initial))
  return {
    getItem: (key: string) => entries.get(key) ?? null,
    setItem: (key: string, value: string) => {
      entries.set(key, value)
    },
    removeItem: (key: string) => {
      entries.delete(key)
    },
  }
}

describe('notification preference storage', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('defaults to on', () => {
    vi.stubGlobal('localStorage', memoryStorage())
    expect(readNotifyEnabled()).toBe(true)
  })

  it('round-trips off and on', () => {
    vi.stubGlobal('localStorage', memoryStorage())
    writeNotifyEnabled(false)
    expect(readNotifyEnabled()).toBe(false)
    writeNotifyEnabled(true)
    expect(readNotifyEnabled()).toBe(true)
  })

  it('survives storage that throws', () => {
    vi.stubGlobal('localStorage', {
      getItem: () => {
        throw new Error('storage denied')
      },
      setItem: () => {
        throw new Error('storage denied')
      },
    })
    expect(readNotifyEnabled()).toBe(true)
    expect(() => writeNotifyEnabled(false)).not.toThrow()
  })
})

describe('sendDesktopNotification', () => {
  it('resolves false outside the Tauri shell instead of throwing', async () => {
    vi.stubGlobal('window', {})
    await expect(sendDesktopNotification({ title: 'Veridicus', body: 'Run finished.' })).resolves.toBe(
      false,
    )
  })
})
