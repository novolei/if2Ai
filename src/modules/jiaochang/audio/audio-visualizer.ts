export interface JiaochangAudioVisualizerFrame {
  bars: number[]
  peak: number
}

export function createSilentVisualizerFrame(size = 16): JiaochangAudioVisualizerFrame {
  return {
    bars: Array.from({ length: size }, () => 0),
    peak: 0,
  }
}

export function createJiaochangVisualizerFrame(seed: number, size = 16): JiaochangAudioVisualizerFrame {
  const bars = Array.from({ length: size }, (_, index) => {
    const wave = Math.sin(seed / 180 + index * 0.72)
    return Math.max(0.08, Math.min(1, (wave + 1) / 2))
  })
  return {
    bars,
    peak: Math.max(...bars),
  }
}
