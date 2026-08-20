import { useMemo } from "react"
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
import { useUiStore } from "@/store/uiStore"

function App() {
  const activePage = useUiStore((state) => state.activePage)
  const setActivePage = useUiStore((state) => state.setActivePage)
  const leftSidebarOpen = useUiStore((state) => state.leftSidebarOpen)
  const setLeftSidebarOpen = useUiStore((state) => state.setLeftSidebarOpen)
  const catalogPanelOpen = useUiStore((state) => state.catalogPanelOpen)
  const setCatalogPanelOpen = useUiStore((state) => state.setCatalogPanelOpen)
  const catalogPanelWidth = useUiStore((state) => state.catalogPanelWidth)
  const setCatalogPanelWidth = useUiStore((state) => state.setCatalogPanelWidth)

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

  return (
    <TooltipProvider>
      <SidebarProvider open={leftSidebarOpen} onOpenChange={setLeftSidebarOpen}>
        <MainSidebar
          side="left"
          activePage={activePage}
          onPageChange={setActivePage}
        />
        <SidebarInset>
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

          <div className="flex min-h-0 flex-1">
            <div className="flex min-h-0 flex-1 flex-col gap-4 p-4">
              <div className="rounded-xl border bg-card p-6">
                <h2 className="text-xl font-semibold">{pageTitle}</h2>
                <p className="mt-2 text-sm text-muted-foreground">
                  Catalog workspace. Next step is wiring this to real folder scan and
                  job actions.
                </p>
                <div className="mt-4 flex flex-wrap gap-2">
                  <Button size="sm">Mark Ready</Button>
                  <Button size="sm" variant="secondary">
                    Assign Platform
                  </Button>
                  <Button size="sm" variant="outline">
                    Schedule
                  </Button>
                </div>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <div className="rounded-xl border bg-card p-6">
                  <h2 className="text-sm font-medium">Ready queue</h2>
                  <p className="mt-2 text-sm text-muted-foreground">
                    Placeholder list for `ready` jobs.
                  </p>
                </div>
                <div className="rounded-xl border bg-card p-6">
                  <h2 className="text-sm font-medium">Scheduled queue</h2>
                  <p className="mt-2 text-sm text-muted-foreground">
                    Placeholder list for `scheduled` jobs.
                  </p>
                </div>
              </div>
            </div>

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
                <CatalogTreePanel className="h-full" />
              </div>
            ) : null}
          </div>
        </SidebarInset>
      </SidebarProvider>
    </TooltipProvider>
  )
}

export default App
