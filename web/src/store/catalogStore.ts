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

export type MediaPreview = {
  reference: string
  resolved_path: string | null
  exists: boolean
  valid_extension: boolean
  error: string | null
}

type CatalogState = {
  roots: CatalogRoot[]
  loading: boolean
  error: string | null
  selectedFilePath: string | null
  selectedFileContent: string
  selectedPublishText: string
  selectedPreviewMedia: MediaPreview[]
  selectedPreviewIssues: string[]
  selectedPreviewPublishable: boolean
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
  selectedPublishText: "",
  selectedPreviewMedia: [],
  selectedPreviewIssues: [],
  selectedPreviewPublishable: false,
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
      selectedPublishText: "",
      selectedPreviewMedia: [],
      selectedPreviewIssues: [],
      selectedPreviewPublishable: false,
    })
    try {
      const [fileResponse, previewResponse] = await Promise.all([
        fetch(`/api/catalog/file?path=${encodeURIComponent(path)}`),
        fetch(`/api/catalog/preview?path=${encodeURIComponent(path)}`),
      ])

      const fileRaw = await fileResponse.text()
      const previewRaw = await previewResponse.text()
      const data = parseApiResponse(fileResponse.status, fileRaw, "catalog file API")
      const previewData = parseApiResponse(
        previewResponse.status,
        previewRaw,
        "catalog preview API"
      )
      if (!fileResponse.ok || !data?.ok) {
        throw new Error(data?.message ?? "failed to load file")
      }
      if (!previewResponse.ok || !previewData?.ok) {
        throw new Error(previewData?.message ?? "failed to load preview")
      }

      const preview = previewData?.preview ?? {}
      set({
        selectedFilePath: data.path ?? path,
        selectedFileContent: data.content ?? "",
        selectedPublishText: preview.publish_text ?? "",
        selectedPreviewMedia: preview.media ?? [],
        selectedPreviewIssues: preview.issues ?? [],
        selectedPreviewPublishable: Boolean(preview.publishable),
        selectedFileLoading: false,
        selectedFileError: null,
      })
    } catch (err) {
      set({
        selectedFilePath: path,
        selectedFileContent: "",
        selectedPublishText: "",
        selectedPreviewMedia: [],
        selectedPreviewIssues: [],
        selectedPreviewPublishable: false,
        selectedFileLoading: false,
        selectedFileError: err instanceof Error ? err.message : "unknown error",
      })
    }
  },
}))

function parseApiResponse(status: number, raw: string, label: string): any {
  let data: any = null
  if (raw.trim().length > 0) {
    try {
      data = JSON.parse(raw)
    } catch {
      throw new Error(`${label} returned non-JSON response (status ${status})`)
    }
  }
  if (!data) {
    throw new Error(`${label} returned empty response (status ${status})`)
  }
  return data
}
