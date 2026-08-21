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
  readyByPath: Record<string, string | null>
  loading: boolean
  error: string | null
  selectedFilePath: string | null
  selectedFileReady: boolean
  selectedFileReadyOperator: string | null
  selectedFileReadyJobId: string | null
  readyActionLoading: boolean
  selectedFileContent: string
  selectedPublishText: string
  selectedPreviewMedia: MediaPreview[]
  selectedPreviewIssues: string[]
  selectedPreviewPublishable: boolean
  selectedFileLoading: boolean
  selectedFileError: string | null
  loadCatalog: () => Promise<void>
  selectFile: (path: string) => Promise<void>
  markSelectedReady: () => Promise<void>
  unmarkSelectedReady: () => Promise<void>
}

const LAST_SELECTED_FILE_STORAGE_KEY = "publo.catalog.lastSelectedFilePath"

export const useCatalogStore = create<CatalogState>((set, get) => ({
  roots: [],
  readyByPath: {},
  loading: false,
  error: null,
  selectedFilePath: null,
  selectedFileReady: false,
  selectedFileReadyOperator: null,
  selectedFileReadyJobId: null,
  readyActionLoading: false,
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
      const roots: CatalogRoot[] = data.roots ?? []
      const readyStates = Array.isArray(data.ready_states) ? data.ready_states : []
      const readyByPath: Record<string, string | null> = {}
      for (const item of readyStates) {
        if (!item || typeof item.path !== "string" || item.path.length === 0) continue
        readyByPath[item.path] = typeof item.operator === "string" ? item.operator : null
      }
      set({
        roots,
        readyByPath,
        loading: false,
        error: null,
      })

      const candidatePath = get().selectedFilePath ?? readLastSelectedFilePath()
      if (candidatePath && treeHasFilePath(roots, candidatePath)) {
        await get().selectFile(candidatePath)
      } else if (candidatePath) {
        clearLastSelectedFilePath()
      }
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "unknown error"
      set({
        roots: [],
        readyByPath: {},
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
      selectedFileReady: false,
      selectedFileReadyOperator: null,
      selectedFileReadyJobId: null,
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
      const ready = data?.ready ?? {}
      const resolvedPath = data.path ?? path
      const readyOperator =
        typeof ready.operator === "string" && ready.operator.length > 0
          ? ready.operator
          : null
      set({
        selectedFilePath: resolvedPath,
        selectedFileContent: data.content ?? "",
        selectedPublishText: preview.publish_text ?? "",
        selectedPreviewMedia: preview.media ?? [],
        selectedPreviewIssues: preview.issues ?? [],
        selectedPreviewPublishable: Boolean(preview.publishable),
        selectedFileReady: Boolean(ready.is_ready),
        selectedFileReadyOperator: readyOperator,
        selectedFileReadyJobId:
          typeof ready.job_id === "string" && ready.job_id.length > 0
            ? ready.job_id
            : null,
        readyByPath: Boolean(ready.is_ready)
          ? {
              ...get().readyByPath,
              [resolvedPath]: readyOperator,
            }
          : Object.fromEntries(
              Object.entries(get().readyByPath).filter(([item]) => item !== resolvedPath)
            ),
        selectedFileLoading: false,
        selectedFileError: null,
      })
      saveLastSelectedFilePath(resolvedPath)
    } catch (err) {
      set({
        selectedFilePath: path,
        selectedFileContent: "",
        selectedPublishText: "",
        selectedPreviewMedia: [],
        selectedPreviewIssues: [],
        selectedPreviewPublishable: false,
        selectedFileReady: false,
        selectedFileReadyOperator: null,
        selectedFileReadyJobId: null,
        selectedFileLoading: false,
        selectedFileError: err instanceof Error ? err.message : "unknown error",
      })
      clearLastSelectedFilePath()
    }
  },
  markSelectedReady: async () => {
    const path = get().selectedFilePath
    if (!path) return

    set({ readyActionLoading: true, selectedFileError: null })
    try {
      const response = await fetch("/api/jobs/ready/mark", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ path }),
      })
      const raw = await response.text()
      const data = parseApiResponse(response.status, raw, "ready mark API")
      if (!response.ok || !data?.ok) {
        throw new Error(data?.message ?? "failed to mark ready")
      }

      const resolvedPath =
        typeof data.path === "string" && data.path.length > 0 ? data.path : path
      set((state) => ({
        readyActionLoading: false,
        selectedFilePath: resolvedPath,
        selectedFileReady: true,
        selectedFileReadyOperator: "user",
        selectedFileReadyJobId:
          typeof data.job_id === "string" && data.job_id.length > 0
            ? data.job_id
            : state.selectedFileReadyJobId,
        readyByPath: {
          ...state.readyByPath,
          [resolvedPath]: "user",
        },
      }))
    } catch (err) {
      set({
        readyActionLoading: false,
        selectedFileError: err instanceof Error ? err.message : "unknown error",
      })
    }
  },
  unmarkSelectedReady: async () => {
    const path = get().selectedFilePath
    if (!path) return

    set({ readyActionLoading: true, selectedFileError: null })
    try {
      const response = await fetch("/api/jobs/ready/unmark", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ path }),
      })
      const raw = await response.text()
      const data = parseApiResponse(response.status, raw, "ready unmark API")
      if (!response.ok || !data?.ok) {
        throw new Error(data?.message ?? "failed to unmark ready")
      }

      const resolvedPath =
        typeof data.path === "string" && data.path.length > 0 ? data.path : path
      set((state) => ({
        readyActionLoading: false,
        selectedFileReady: false,
        selectedFileReadyOperator: null,
        selectedFileReadyJobId: null,
        readyByPath: Object.fromEntries(
          Object.entries(state.readyByPath).filter(
            ([item]) => item !== path && item !== resolvedPath
          )
        ),
      }))
    } catch (err) {
      set({
        readyActionLoading: false,
        selectedFileError: err instanceof Error ? err.message : "unknown error",
      })
    }
  },
}))

function treeHasFilePath(roots: CatalogRoot[], targetPath: string): boolean {
  for (const root of roots) {
    if (!root.ok) continue
    if (nodeListHasPath(root.tree, targetPath)) return true
  }
  return false
}

function nodeListHasPath(nodes: CatalogNode[], targetPath: string): boolean {
  for (const node of nodes) {
    if (node.kind === "file" && node.path === targetPath) return true
    if (node.children && node.children.length > 0) {
      if (nodeListHasPath(node.children, targetPath)) return true
    }
  }
  return false
}

function readLastSelectedFilePath(): string | null {
  try {
    const value = window.localStorage.getItem(LAST_SELECTED_FILE_STORAGE_KEY)
    if (!value) return null
    const trimmed = value.trim()
    return trimmed.length > 0 ? trimmed : null
  } catch {
    return null
  }
}

function saveLastSelectedFilePath(path: string): void {
  try {
    window.localStorage.setItem(LAST_SELECTED_FILE_STORAGE_KEY, path)
  } catch {
    // ignore storage failures
  }
}

function clearLastSelectedFilePath(): void {
  try {
    window.localStorage.removeItem(LAST_SELECTED_FILE_STORAGE_KEY)
  } catch {
    // ignore storage failures
  }
}

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
