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
  selectedFilePath: string | null
  selectedFileContent: string
  selectedFileLoading: boolean
  selectedFileError: string | null
  loadCatalog: () => Promise<void>
  selectFile: (path: string) => Promise<void>
}

export const useCatalogStore = create<CatalogState>((set) => ({
  roots: [],
  loading: false,
  error: null,
  selectedFilePath: null,
  selectedFileContent: "",
  selectedFileLoading: false,
  selectedFileError: null,
  loadCatalog: async () => {
    set({ loading: true, error: null })
    try {
      const response = await fetch("/api/catalog/tree")
      const raw = await response.text()
      let data: any = null
      if (raw.trim().length > 0) {
        try {
          data = JSON.parse(raw)
        } catch {
          throw new Error(
            `catalog API returned non-JSON response (status ${response.status})`
          )
        }
      }
      if (!data) {
        throw new Error(`catalog API returned empty response (status ${response.status})`)
      }
      if (!response.ok || !data?.ok) {
        throw new Error(data?.message ?? "failed to load catalog")
      }
      set({
        roots: data.roots ?? [],
        loading: false,
        error: null,
      })
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "unknown error"
      set({
        roots: [],
        loading: false,
        error:
          message.includes("Failed to fetch")
            ? "Cannot reach Publo API. Start backend with `cargo run -- serve` (or `pnpm --dir web dev:full`)."
            : message,
      })
    }
  },
  selectFile: async (path) => {
    set({
      selectedFilePath: path,
      selectedFileLoading: true,
      selectedFileError: null,
      selectedFileContent: "",
    })
    try {
      const response = await fetch(`/api/catalog/file?path=${encodeURIComponent(path)}`)
      const raw = await response.text()
      let data: any = null
      if (raw.trim().length > 0) {
        try {
          data = JSON.parse(raw)
        } catch {
          throw new Error(
            `catalog file API returned non-JSON response (status ${response.status})`
          )
        }
      }
      if (!data) {
        throw new Error(`catalog file API returned empty response (status ${response.status})`)
      }
      if (!response.ok || !data?.ok) {
        throw new Error(data?.message ?? "failed to load file")
      }
      set({
        selectedFilePath: data.path ?? path,
        selectedFileContent: data.content ?? "",
        selectedFileLoading: false,
        selectedFileError: null,
      })
    } catch (err) {
      set({
        selectedFilePath: path,
        selectedFileContent: "",
        selectedFileLoading: false,
        selectedFileError: err instanceof Error ? err.message : "unknown error",
      })
    }
  },
}))
