import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  MIN_CHIME_RUN_MS,
  readChimeMode,
  shouldChime,
  writeChimeMode,
  type ChimeMode,
} from './completionSound'

type Policy = Parameters<typeof shouldChime>[0]

const LONG_RUN_MS = MIN_CHIME_RUN_MS + 1_000

/** Backgrounded, long run under the default preference — the case that should chime. */
function policy(overrides: Partial<Policy>): Policy {
  return { mode: 'background', backgrounded: true, durationMs: LONG_RUN_MS, ...overrides }
}

describe('shouldChime', () => {
  it('stays silent when the preference is off', () => {
    expect(shouldChime(policy({ mode: 'off' }))).toBe(false)
    expect(shouldChime(policy({ mode: 'off', backgrounded: false, durationMs: 600_000 }))).toBe(false)
  })

  it('stays silent in background mode while the window is focused', () => {
    expect(shouldChime(policy({ backgrounded: false }))).toBe(false)
    expect(shouldChime(policy({ backgrounded: false, durationMs: 600_000 }))).toBe(false)
  })

  it('requires the eight-second floor', () => {
    expect(shouldChime(policy({ durationMs: 0 }))).toBe(false)
    expect(shouldChime(policy({ durationMs: MIN_CHIME_RUN_MS - 1 }))).toBe(false)
    expect(shouldChime(policy({ durationMs: MIN_CHIME_RUN_MS }))).toBe(true)
    expect(shouldChime(policy({ durationMs: MIN_CHIME_RUN_MS + 1 }))).toBe(true)
  })

  it('chimes for a focused window in always mode, but still not for short runs', () => {
    expect(shouldChime(policy({ mode: 'always', backgrounded: false }))).toBe(true)
    expect(shouldChime(policy({ mode: 'always', backgrounded: false, durationMs: 3_000 }))).toBe(false)
  })

  it('never chimes for a cancelled run', () => {
    expect(shouldChime(policy({ cancelled: true }))).toBe(false)
    expect(shouldChime(policy({ mode: 'always', cancelled: true }))).toBe(false)
  })

  it('accepts a per-call floor for sessions that are long by design', () => {
    expect(shouldChime(policy({ minDurationMs: 0, durationMs: 0 }))).toBe(true)
    expect(shouldChime(policy({ mode: 'always', backgrounded: false, minDurationMs: 0, durationMs: 0 }))).toBe(true)
    // A zero floor removes the timing test, not the background test.
    expect(shouldChime(policy({ backgrounded: false, minDurationMs: 0, durationMs: 0 }))).toBe(false)
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
    clear: () => entries.clear(),
    key: (index: number) => Array.from(entries.keys())[index] ?? null,
    get length() {
      return entries.size
    },
  }
}

describe('chime preference storage', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('defaults to background when nothing is stored', () => {
    vi.stubGlobal('localStorage', memoryStorage())
    expect(readChimeMode()).toBe('background')
  })

  it('round-trips every mode', () => {
    vi.stubGlobal('localStorage', memoryStorage())
    for (const mode of ['always', 'off', 'background'] as ChimeMode[]) {
      writeChimeMode(mode)
      expect(readChimeMode()).toBe(mode)
    }
  })

  it('ignores a value it does not recognise', () => {
    vi.stubGlobal('localStorage', memoryStorage({ 'careeros:completion-sound': 'loud' }))
    expect(readChimeMode()).toBe('background')
  })

  it('still returns the default when storage throws', () => {
    vi.stubGlobal('localStorage', {
      getItem: () => {
        throw new Error('storage denied')
      },
      setItem: () => {
        throw new Error('storage denied')
      },
    })
    expect(readChimeMode()).toBe('background')
    expect(() => writeChimeMode('always')).not.toThrow()
  })
})
