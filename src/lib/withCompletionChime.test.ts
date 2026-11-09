import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { withCompletionChime } from './completionSound'

/**
 * The wrapper's contract is only observable through the audio it produces, so
 * these tests run it against a fake AudioContext and record the frequencies it
 * starts. A success chime is 660 Hz then 880 Hz; a failure is a single 330 Hz.
 */

const started: number[] = []

class FakeAudioContext {
  state = 'running'
  currentTime = 0
  destination = {} as AudioDestinationNode

  createOscillator() {
    const oscillator = {
      type: 'sine',
      frequency: { value: 0 },
      connect: () => {},
      start: () => started.push(oscillator.frequency.value),
      stop: () => {},
    }
    return oscillator as unknown as OscillatorNode
  }

  createGain() {
    return {
      gain: {
        setValueAtTime: () => {},
        linearRampToValueAtTime: () => {},
        exponentialRampToValueAtTime: () => {},
      },
      connect: () => {},
    } as unknown as GainNode
  }

  resume() {
    return Promise.resolve()
  }
}

function memoryStorage(mode: string) {
  return {
    getItem: () => mode,
    setItem: () => {},
    removeItem: () => {},
  }
}

// Each test starts a minute after the last so the 400 ms burst guard from the
// previous test cannot swallow this one's chime.
let clock = 1_700_000_000_000

function backgroundedWindow() {
  vi.stubGlobal('document', { hidden: true, hasFocus: () => false })
}

beforeEach(() => {
  clock += 60_000
  vi.useFakeTimers({ now: clock })
  started.length = 0
  vi.stubGlobal('window', { AudioContext: FakeAudioContext })
  vi.stubGlobal('localStorage', memoryStorage('background'))
  backgroundedWindow()
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('withCompletionChime', () => {
  it('returns the value and chimes for a long background run', async () => {
    const value = await withCompletionChime(async () => {
      vi.advanceTimersByTime(9_000)
      return 'pack'
    })

    expect(value).toBe('pack')
    expect(started).toEqual([660, 880])
  })

  it('stays silent below the eight-second floor', async () => {
    await withCompletionChime(async () => {
      vi.advanceTimersByTime(5_000)
      return 'quick'
    })

    expect(started).toEqual([])
  })

  it('uses the lower tone when the resolved value counts as a failure', async () => {
    await withCompletionChime(
      async () => {
        vi.advanceTimersByTime(9_000)
        return { success: false }
      },
      { failed: (value) => !value.success },
    )

    expect(started).toEqual([330])
  })

  it('uses the lower tone and rethrows when the run rejects', async () => {
    await expect(
      withCompletionChime(async () => {
        vi.advanceTimersByTime(9_000)
        throw new Error('ollama unreachable')
      }),
    ).rejects.toThrow('ollama unreachable')

    expect(started).toEqual([330])
  })

  it('stays silent for a cancelled outcome', async () => {
    await withCompletionChime(
      async () => {
        vi.advanceTimersByTime(9_000)
        return { error_message: 'Cancelled by user' }
      },
      { cancelled: (value) => /cancelled by user/i.test(value.error_message) },
    )

    expect(started).toEqual([])
  })

  it('honours a zero floor for sessions that are long by design', async () => {
    await withCompletionChime(async () => 0, { minDurationMs: 0 })

    expect(started).toEqual([660, 880])
  })

  it('respects the mode: no chime while focused in background mode', async () => {
    vi.stubGlobal('document', { hidden: false, hasFocus: () => true })

    await withCompletionChime(async () => {
      vi.advanceTimersByTime(9_000)
      return 'done'
    })

    expect(started).toEqual([])
  })
})
