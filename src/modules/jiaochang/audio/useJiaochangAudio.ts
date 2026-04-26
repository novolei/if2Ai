import { useContext } from 'react'

import { JiaochangAudioContext } from './JiaochangAudioProvider.tsx'

export function useJiaochangAudio() {
  const value = useContext(JiaochangAudioContext)
  if (!value) {
    throw new Error('useJiaochangAudio must be used inside JiaochangAudioProvider')
  }
  return value
}
