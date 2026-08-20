import { useEffect, useRef, useState } from "react"
import { useCatalogStore } from "@/store/catalogStore"

export function AppInitializer({ children }: { children: React.ReactNode }) {
  const [ready, setReady] = useState(false)
  const loadCatalog = useCatalogStore((state) => state.loadCatalog)
  const initialized = useRef(false)

  useEffect(() => {
    if (initialized.current) return
    initialized.current = true
    loadCatalog().finally(() => setReady(true))
  }, [loadCatalog])

  if (!ready) return null
  return <>{children}</>
}
