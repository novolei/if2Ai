import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'
import { listProceduralEntries } from '@/api/memory'
import type { ProceduralEntryDto } from '@/transport/contracts'

export function useProceduralEntries() {
  const [entries, setEntries] = useState<ProceduralEntryDto[] | null>(null)

  const refresh = useCallback(async () => {
    try {
      const list = await listProceduralEntries()
      setEntries(list)
    } catch (err) {
      toast.error(`Failed to load procedural entries: ${err}`)
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  return { entries, refresh }
}
