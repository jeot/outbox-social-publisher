import { useEffect, useRef, useState } from "react"
import { FileTree } from "@/components/file-tree"
import { RawFileCard } from "@/components/raw-file-card"
import { useCatalogStore } from "@/store/catalogStore"
import { useUiStore } from "@/store/uiStore"

const TREE_PANE_MIN_WIDTH = 280
const RAW_PANE_MIN_WIDTH = 320
const PANE_DIVIDER_WIDTH = 16
const PANE_GAP = 16
const TOTAL_HORIZONTAL_GAP = PANE_GAP * 2
const CONTENT_SIDE_PADDING = 32

export function CatalogPage() {
  const catalogTreePaneWidth = useUiStore((state) => state.catalogPreviewPaneWidth)
  const setCatalogTreePaneWidth = useUiStore(
    (state) => state.setCatalogPreviewPaneWidth
  )

  const selectedFilePath = useCatalogStore((state) => state.selectedFilePath)
  const selectedFileContent = useCatalogStore((state) => state.selectedFileContent)
  const selectedFileReady = useCatalogStore((state) => state.selectedFileReady)
  const readyActionLoading = useCatalogStore((state) => state.readyActionLoading)
  const markSelectedReady = useCatalogStore((state) => state.markSelectedReady)
  const unmarkSelectedReady = useCatalogStore((state) => state.unmarkSelectedReady)
  const selectedFileLoading = useCatalogStore((state) => state.selectedFileLoading)
  const selectedFileError = useCatalogStore((state) => state.selectedFileError)

  const catalogContentRef = useRef<HTMLDivElement | null>(null)
  const [catalogContentWidth, setCatalogContentWidth] = useState(0)

  const availableCatalogWidth = Math.max(0, catalogContentWidth - CONTENT_SIDE_PADDING)
  const canUseHorizontalCatalogLayout =
    availableCatalogWidth >=
    TREE_PANE_MIN_WIDTH + RAW_PANE_MIN_WIDTH + PANE_DIVIDER_WIDTH + TOTAL_HORIZONTAL_GAP
  const maxTreeWidth = Math.max(
    TREE_PANE_MIN_WIDTH,
    availableCatalogWidth - RAW_PANE_MIN_WIDTH - PANE_DIVIDER_WIDTH - TOTAL_HORIZONTAL_GAP
  )
  const effectiveTreePaneWidth = Math.max(
    TREE_PANE_MIN_WIDTH,
    Math.min(catalogTreePaneWidth, maxTreeWidth)
  )

  useEffect(() => {
    const node = catalogContentRef.current
    if (!node) return

    const update = () => setCatalogContentWidth(node.clientWidth)
    update()

    const observer = new ResizeObserver(() => update())
    observer.observe(node)

    return () => observer.disconnect()
  }, [])

  return (
    <div className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto">
      <div
        ref={catalogContentRef}
        className={`flex min-h-full w-full p-4 ${canUseHorizontalCatalogLayout ? "flex-row items-start gap-1" : "flex-col gap-4"}`}
      >
        <section
          className={`overflow-hidden rounded-xl border bg-card ${canUseHorizontalCatalogLayout ? "shrink-0" : "flex-1"}`}
          style={
            canUseHorizontalCatalogLayout
              ? { width: `min(100%, ${effectiveTreePaneWidth}px)` }
              : undefined
          }
        >
          <FileTree />
        </section>

        <div
          className={`relative w-4 shrink-0 ${canUseHorizontalCatalogLayout ? "sticky top-0 block h-[calc(100svh-3.5rem)]" : "hidden"}`}
        >
          <button
            type="button"
            aria-label="Resize preview panel"
            className="group absolute inset-0 cursor-col-resize"
            onMouseDown={(event) => {
              event.preventDefault()
              const startX = event.clientX
              const startWidth = effectiveTreePaneWidth
              const onMove = (moveEvent: MouseEvent) => {
                const delta = moveEvent.clientX - startX
                const next = Math.max(
                  TREE_PANE_MIN_WIDTH,
                  Math.min(maxTreeWidth, startWidth + delta)
                )
                setCatalogTreePaneWidth(next)
              }
              const onUp = () => {
                window.removeEventListener("mousemove", onMove)
                window.removeEventListener("mouseup", onUp)
              }
              window.addEventListener("mousemove", onMove)
              window.addEventListener("mouseup", onUp)
            }}
          >
            <span className="pointer-events-none absolute inset-y-0 left-1/2 w-px -translate-x-1/2 group-hover:bg-border" />
            <span className="pointer-events-none absolute top-1/2 left-1/2 flex -translate-x-1/2 -translate-y-1/2 flex-col gap-1">
              <span className="size-1 rounded-full bg-muted-foreground/60" />
              <span className="size-1 rounded-full bg-muted-foreground/60" />
              <span className="size-1 rounded-full bg-muted-foreground/60" />
            </span>
          </button>
        </div>

        <RawFileCard
          selectedFilePath={selectedFilePath}
          selectedFileReady={selectedFileReady}
          readyActionLoading={readyActionLoading}
          onToggleReady={() => {
            if (selectedFileReady) {
              void unmarkSelectedReady()
              return
            }
            void markSelectedReady()
          }}
          selectedFileLoading={selectedFileLoading}
          selectedFileError={selectedFileError}
          selectedFileContent={selectedFileContent}
          className={`flex min-w-0 flex-1 flex-col rounded-xl border bg-card ${canUseHorizontalCatalogLayout ? "min-w-[320px]" : "min-w-0"}`}
        />
      </div>
    </div>
  )
}
