import { create } from "zustand"

export type CatalogNode = {
  name: string
  path: string
  kind: "file" | "dir"
  children?: CatalogNode[]
}

export type CatalogRoot = {
  root: string
  ok: boolean
  tree: CatalogNode[]
  error?: string
}

type CatalogState = {
  roots: CatalogRoot[]
  loading: boolean
  error: string | null
  loadCatalog: () => Promise<void>
}

export const useCatalogStore = create<CatalogState>((set) => ({
  roots: [],
  loading: false,
  error: null,
  loadCatalog: async () => {
    set({ loading: true, error: null })
    try {
      const response = await fetch("/api/catalog/tree")
      const data = await response.json()
      if (!response.ok || !data?.ok) {
        throw new Error(data?.message ?? "failed to load catalog")
      }
      set({
        roots: data.roots ?? [],
        loading: false,
        error: null,
      })
    } catch (err) {
      set({
        roots: [],
        loading: false,
        error: err instanceof Error ? err.message : "unknown error",
      })
    }
  },
}))
