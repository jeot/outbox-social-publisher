import type { ReactNode } from "react"

type RightSidebarProps = {
  width: number
  onWidthChange: (width: number) => void
  minWidth?: number
  maxWidth?: number
  children: ReactNode
}

export function RightSidebar({
  width,
  onWidthChange,
  minWidth = 260,
  maxWidth = 640,
  children,
}: RightSidebarProps) {
  return (
    <div className="relative min-h-0 shrink-0" style={{ width: `${width}px` }}>
      <div
        role="separator"
        aria-orientation="vertical"
        className="absolute top-0 left-0 z-10 h-full w-1 cursor-col-resize bg-transparent hover:bg-border"
        onMouseDown={(event) => {
          event.preventDefault()
          const startX = event.clientX
          const startWidth = width
          const onMove = (moveEvent: MouseEvent) => {
            const delta = startX - moveEvent.clientX
            const next = Math.max(minWidth, Math.min(maxWidth, startWidth + delta))
            onWidthChange(next)
          }
          const onUp = () => {
            window.removeEventListener("mousemove", onMove)
            window.removeEventListener("mouseup", onUp)
          }
          window.addEventListener("mousemove", onMove)
          window.addEventListener("mouseup", onUp)
        }}
      />
      <div className="h-full min-h-0 overflow-hidden">{children}</div>
    </div>
  )
}
