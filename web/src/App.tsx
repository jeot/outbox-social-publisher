import { useEffect, useMemo, useRef, useState } from "react"
import { PanelRightClose, PanelRightOpen } from "lucide-react"
import { CatalogTreePanel } from "@/components/catalog-tree-sidebar"
import { MainSidebar } from "@/components/main-sidebar"
import { Button } from "@/components/ui/button"
import { Separator } from "@/components/ui/separator"
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useCatalogStore } from "@/store/catalogStore"
import { useUiStore } from "@/store/uiStore"

function App() {
  const PREVIEW_PANE_MIN_WIDTH = 320
  const RAW_PANE_MIN_WIDTH = 320
  const PANE_DIVIDER_WIDTH = 16
  const PANE_GAP = 16
  const TOTAL_HORIZONTAL_GAP = PANE_GAP * 2
  const CONTENT_SIDE_PADDING = 32 // p-4 => 16px left + 16px right
  const activePage = useUiStore((state) => state.activePage)
  const setActivePage = useUiStore((state) => state.setActivePage)
  const leftSidebarOpen = useUiStore((state) => state.leftSidebarOpen)
  const setLeftSidebarOpen = useUiStore((state) => state.setLeftSidebarOpen)
  const catalogPanelOpen = useUiStore((state) => state.catalogPanelOpen)
  const setCatalogPanelOpen = useUiStore((state) => state.setCatalogPanelOpen)
  const catalogPanelWidth = useUiStore((state) => state.catalogPanelWidth)
  const setCatalogPanelWidth = useUiStore((state) => state.setCatalogPanelWidth)
  const catalogPreviewPaneWidth = useUiStore((state) => state.catalogPreviewPaneWidth)
  const setCatalogPreviewPaneWidth = useUiStore(
    (state) => state.setCatalogPreviewPaneWidth
  )
  const selectedFilePath = useCatalogStore((state) => state.selectedFilePath)
  const selectedFileContent = useCatalogStore((state) => state.selectedFileContent)
  const selectedFileLoading = useCatalogStore((state) => state.selectedFileLoading)
  const selectedFileError = useCatalogStore((state) => state.selectedFileError)
  const catalogContentRef = useRef<HTMLDivElement | null>(null)
  const [catalogContentWidth, setCatalogContentWidth] = useState(0)

  const pageTitle = useMemo(() => {
    switch (activePage) {
      case "ready":
        return "Ready"
      case "scheduled":
        return "Scheduled"
      default:
        return "Catalog"
    }
  }, [activePage])

  const showCatalogPanel = activePage === "catalog" && catalogPanelOpen
  const availableCatalogWidth = Math.max(0, catalogContentWidth - CONTENT_SIDE_PADDING)
  const canUseHorizontalCatalogLayout =
    availableCatalogWidth >=
    PREVIEW_PANE_MIN_WIDTH + RAW_PANE_MIN_WIDTH + PANE_DIVIDER_WIDTH + TOTAL_HORIZONTAL_GAP
  const maxPreviewWidth = Math.max(
    PREVIEW_PANE_MIN_WIDTH,
    availableCatalogWidth - RAW_PANE_MIN_WIDTH - PANE_DIVIDER_WIDTH - TOTAL_HORIZONTAL_GAP
  )
  const effectivePreviewPaneWidth = Math.max(
    PREVIEW_PANE_MIN_WIDTH,
    Math.min(catalogPreviewPaneWidth, maxPreviewWidth)
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
    <TooltipProvider>
      <SidebarProvider
        className="h-svh overflow-hidden"
        open={leftSidebarOpen}
        onOpenChange={setLeftSidebarOpen}
      >
        <MainSidebar
          side="left"
          activePage={activePage}
          onPageChange={setActivePage}
        />
        <SidebarInset className="h-svh min-h-0 overflow-hidden">
          <header className="flex h-14 shrink-0 items-center gap-2 border-b bg-background px-4">
            <SidebarTrigger />
            <Separator orientation="vertical" className="h-4" />
            <h1 className="text-base font-bold tracking-tight">{pageTitle}</h1>
            {activePage === "catalog" ? (
              <>
                <div className="ml-auto" />
                <Button
                  type="button"
                  size="icon-sm"
                  variant="ghost"
                  onClick={() => setCatalogPanelOpen(!catalogPanelOpen)}
                  title={catalogPanelOpen ? "Hide catalog panel" : "Show catalog panel"}
                >
                  {catalogPanelOpen ? <PanelRightClose /> : <PanelRightOpen />}
                </Button>
              </>
            ) : null}
          </header>

          <div className="flex min-h-0 flex-1 overflow-hidden">
            {activePage === "catalog" ? (
              <div
                ref={catalogContentRef}
                className={`flex min-h-0 flex-1 gap-4 overflow-hidden p-4 ${canUseHorizontalCatalogLayout ? "flex-row" : "flex-col"
                  }`}
              >
                <section
                  className={`flex min-h-0 flex-col rounded-xl border bg-card ${canUseHorizontalCatalogLayout ? "shrink-0" : "flex-1"
                    }`}
                  style={
                    canUseHorizontalCatalogLayout
                      ? { width: `min(100%, ${effectivePreviewPaneWidth}px)` }
                      : undefined
                  }
                >
                  <div className="border-b px-4 py-3">
                    <h2 className="text-sm font-semibold">Publish Preview (next)</h2>
                    <p className="text-xs text-muted-foreground">
                      Parsed text + media preview will be added next.
                    </p>
                  </div>
                  <div className="flex-1 overflow-auto p-4">
                    <p className="text-sm text-muted-foreground">
                      Select a file from Catalog to preview raw content first.
                    </p>
                  </div>
                </section>

                <div
                  className={`relative w-4 shrink-0 ${canUseHorizontalCatalogLayout ? "block" : "hidden"
                    }`}
                >
                  <button
                    type="button"
                    aria-label="Resize preview panel"
                    className="group absolute inset-0 cursor-col-resize"
                    onMouseDown={(event) => {
                      event.preventDefault()
                      const startX = event.clientX
                      const startWidth = effectivePreviewPaneWidth
                      const onMove = (moveEvent: MouseEvent) => {
                        const delta = moveEvent.clientX - startX
                        const next = Math.max(
                          PREVIEW_PANE_MIN_WIDTH,
                          Math.min(maxPreviewWidth, startWidth + delta)
                        )
                        setCatalogPreviewPaneWidth(next)
                      }
                      const onUp = () => {
                        window.removeEventListener("mousemove", onMove)
                        window.removeEventListener("mouseup", onUp)
                      }
                      window.addEventListener("mousemove", onMove)
                      window.addEventListener("mouseup", onUp)
                    }}
                  >
                    <span className="pointer-events-none absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-border/70 group-hover:bg-border" />
                    <span className="pointer-events-none absolute top-1/2 left-1/2 flex -translate-x-1/2 -translate-y-1/2 flex-col gap-1">
                      <span className="size-1 rounded-full bg-muted-foreground/60" />
                      <span className="size-1 rounded-full bg-muted-foreground/60" />
                      <span className="size-1 rounded-full bg-muted-foreground/60" />
                    </span>
                  </button>
                </div>

                <section
                  className={`flex min-h-0 flex-1 flex-col rounded-xl border bg-card ${canUseHorizontalCatalogLayout ? "min-w-[320px]" : "min-w-0"
                    }`}
                >
                  <div className="border-b px-4 py-3">
                    <h2 className="text-sm font-semibold">Raw File Content</h2>
                    <p className="mt-1 truncate text-xs text-muted-foreground">
                      {selectedFilePath ?? "No file selected"}
                    </p>
                  </div>
                  <div className="flex-1 overflow-auto p-4">
                    {selectedFileLoading ? (
                      <p className="text-sm text-muted-foreground">Loading file…</p>
                    ) : selectedFileError ? (
                      <p className="text-sm text-destructive">{selectedFileError}</p>
                    ) : selectedFilePath ? (
                      <pre className="whitespace-pre-wrap break-words rounded-md bg-muted p-3 text-sm leading-relaxed">
                        {selectedFileContent}
                      </pre>
                    ) : (
                      <p className="text-sm text-muted-foreground">
                        Select a markdown file in Catalog.
                      </p>
                    )}
                  </div>
                </section>
              </div>
            ) : (
              <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-4">
                <div className="rounded-xl border bg-card p-6">
                  <h2 className="text-xl font-semibold">{pageTitle}</h2>
                  <p className="mt-2 text-sm text-muted-foreground">
                    {pageTitle} workspace is in progress.
                  </p>
                </div>
              </div>
            )}

            {showCatalogPanel ? (
              <div
                className="relative min-h-0 shrink-0"
                style={{ width: `${catalogPanelWidth}px` }}
              >
                <div
                  role="separator"
                  aria-orientation="vertical"
                  className="absolute top-0 left-0 z-10 h-full w-1 cursor-col-resize bg-transparent hover:bg-border"
                  onMouseDown={(event) => {
                    event.preventDefault()
                    const startX = event.clientX
                    const startWidth = catalogPanelWidth
                    const onMove = (moveEvent: MouseEvent) => {
                      const delta = startX - moveEvent.clientX
                      const next = Math.max(260, Math.min(640, startWidth + delta))
                      setCatalogPanelWidth(next)
                    }
                    const onUp = () => {
                      window.removeEventListener("mousemove", onMove)
                      window.removeEventListener("mouseup", onUp)
                    }
                    window.addEventListener("mousemove", onMove)
                    window.addEventListener("mouseup", onUp)
                  }}
                />
                <CatalogTreePanel className="h-full min-h-0 overflow-hidden" />
              </div>
            ) : null}
          </div>
        </SidebarInset>
      </SidebarProvider>
    </TooltipProvider>
  )
}

export default App
