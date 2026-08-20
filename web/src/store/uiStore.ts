import { create } from "zustand"
import { persist } from "zustand/middleware"
import type { AppPage } from "@/components/main-sidebar"

type UiState = {
  activePage: AppPage
  leftSidebarOpen: boolean
  catalogPanelOpen: boolean
  catalogPanelWidth: number
  setActivePage: (page: AppPage) => void
  setLeftSidebarOpen: (open: boolean) => void
  setCatalogPanelOpen: (open: boolean) => void
  setCatalogPanelWidth: (width: number) => void
}

export const useUiStore = create<UiState>()(
  persist(
    (set) => ({
      activePage: "catalog",
      leftSidebarOpen: true,
      catalogPanelOpen: true,
      catalogPanelWidth: 360,
      setActivePage: (page) => set({ activePage: page }),
      setLeftSidebarOpen: (open) => set({ leftSidebarOpen: open }),
      setCatalogPanelOpen: (open) => set({ catalogPanelOpen: open }),
      setCatalogPanelWidth: (width) => set({ catalogPanelWidth: width }),
    }),
    {
      name: "publo-ui",
      partialize: (state) => ({
        activePage: state.activePage,
        leftSidebarOpen: state.leftSidebarOpen,
        catalogPanelOpen: state.catalogPanelOpen,
        catalogPanelWidth: state.catalogPanelWidth,
      }),
    }
  )
)
