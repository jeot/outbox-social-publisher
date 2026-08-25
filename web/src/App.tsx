import { useMemo } from "react"
import { AppHeader } from "@/components/app-header"
import { AppSidebar, type AppPage } from "@/components/app-sidebar"
import { CatalogPage } from "@/components/catalog-page"
import { FileTree } from "@/components/file-tree"
import { DecisionPage } from "@/components/decision-page"
import { RightSidebar } from "@/components/right-sidebar"
import { ScheduledPage } from "@/components/scheduled-page"
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useUiStore } from "@/store/uiStore"
import { Toaster } from "sonner"

function App() {
  const activePage = useUiStore((state) => state.activePage)
  const setActivePage = useUiStore((state) => state.setActivePage)
  const leftSidebarOpen = useUiStore((state) => state.leftSidebarOpen)
  const setLeftSidebarOpen = useUiStore((state) => state.setLeftSidebarOpen)
  const catalogPanelOpen = useUiStore((state) => state.catalogPanelOpen)
  const setCatalogPanelOpen = useUiStore((state) => state.setCatalogPanelOpen)
  const catalogPanelWidth = useUiStore((state) => state.catalogPanelWidth)
  const setCatalogPanelWidth = useUiStore((state) => state.setCatalogPanelWidth)

  const pageTitle = useMemo(() => getPageTitle(activePage), [activePage])
  const showCatalogPanel = activePage === "catalog" && catalogPanelOpen

  return (
    <TooltipProvider>
      <Toaster richColors position="bottom-right" />
      <SidebarProvider
        className="h-svh overflow-hidden"
        open={leftSidebarOpen}
        onOpenChange={setLeftSidebarOpen}
      >
        <AppSidebar side="left" activePage={activePage} onPageChange={setActivePage} />
        <SidebarInset className="h-svh min-h-0 overflow-hidden">
          <AppHeader
            title={pageTitle}
            showRightPanelToggle={activePage === "catalog"}
            rightPanelOpen={catalogPanelOpen}
            onToggleRightPanel={() => setCatalogPanelOpen(!catalogPanelOpen)}
          />

          <div className="flex min-h-0 flex-1 overflow-hidden">
            <MainContent activePage={activePage} />
            {showCatalogPanel ? (
              <RightSidebar width={catalogPanelWidth} onWidthChange={setCatalogPanelWidth}>
                <FileTree />
              </RightSidebar>
            ) : null}
          </div>
        </SidebarInset>
      </SidebarProvider>
    </TooltipProvider>
  )
}

function MainContent({ activePage }: { activePage: AppPage }) {
  switch (activePage) {
    case "ready":
      return <DecisionPage />
    case "scheduled":
      return <ScheduledPage />
    default:
      return <CatalogPage />
  }
}

function getPageTitle(activePage: AppPage): string {
  switch (activePage) {
    case "ready":
      return "Decision Queue"
    case "scheduled":
      return "Scheduled"
    default:
      return "Catalog"
  }
}

export default App
