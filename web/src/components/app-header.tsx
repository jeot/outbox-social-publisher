import { PanelRightClose, PanelRightOpen } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Separator } from "@/components/ui/separator"
import { SidebarTrigger } from "@/components/ui/sidebar"

type AppHeaderProps = {
  title: string
  rightPanelOpen: boolean
  onToggleRightPanel: () => void
}

export function AppHeader({
  title,
  rightPanelOpen,
  onToggleRightPanel,
}: AppHeaderProps) {
  return (
    <header className="flex h-14 shrink-0 items-center gap-2 border-b bg-background px-4">
      <SidebarTrigger />
      <Separator orientation="vertical" className="" />
      <h1 className="text-base font-bold tracking-tight">{title}</h1>
      <div className="ml-auto" />
      <Button
        type="button"
        size="icon-sm"
        variant="ghost"
        onClick={onToggleRightPanel}
        title={rightPanelOpen ? "Hide preview panel" : "Show preview panel"}
      >
        {rightPanelOpen ? <PanelRightClose /> : <PanelRightOpen />}
      </Button>
    </header>
  )
}
