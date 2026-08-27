const BELL_SOUND_URL =
  "/sounds/universfield-single-church-bell-2-352062.mp3"

let bellAudio: HTMLAudioElement | null = null
let audioContext: AudioContext | null = null

export async function playReminderBell(): Promise<boolean> {
  try {
    const audio = getBellAudio()
    audio.pause()
    audio.currentTime = 0
    await audio.play()
    return true
  } catch {
    return playSynthesizedBell()
  }
}

function getBellAudio(): HTMLAudioElement {
  if (bellAudio) return bellAudio

  bellAudio = new Audio(BELL_SOUND_URL)
  bellAudio.preload = "auto"
  bellAudio.volume = 0.8
  return bellAudio
}

async function playSynthesizedBell(): Promise<boolean> {
  try {
    const context = getAudioContext()
    if (context.state === "suspended") {
      await context.resume()
    }

    const startedAt = context.currentTime
    const durationSeconds = 1.5
    const masterGain = context.createGain()

    masterGain.gain.setValueAtTime(0.0001, startedAt)
    masterGain.gain.exponentialRampToValueAtTime(0.45, startedAt + 0.008)
    masterGain.gain.exponentialRampToValueAtTime(
      0.0001,
      startedAt + durationSeconds
    )
    masterGain.connect(context.destination)

    const partials = [
      { frequency: 880, gain: 1 },
      { frequency: 1_320, gain: 0.55 },
      { frequency: 1_760, gain: 0.25 },
    ]

    for (const partial of partials) {
      const oscillator = context.createOscillator()
      const partialGain = context.createGain()

      oscillator.type = "sine"
      oscillator.frequency.setValueAtTime(partial.frequency, startedAt)
      oscillator.frequency.exponentialRampToValueAtTime(
        partial.frequency * 0.985,
        startedAt + durationSeconds
      )
      partialGain.gain.setValueAtTime(partial.gain, startedAt)
      partialGain.gain.exponentialRampToValueAtTime(
        0.0001,
        startedAt + durationSeconds
      )

      oscillator.connect(partialGain)
      partialGain.connect(masterGain)
      oscillator.start(startedAt)
      oscillator.stop(startedAt + durationSeconds)
    }

    return context.state === "running"
  } catch {
    return false
  }
}

function getAudioContext(): AudioContext {
  audioContext ??= new AudioContext()
  return audioContext
}
