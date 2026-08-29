import { cn } from "@/lib/utils"
import { useState } from "react"
import { Button } from "./ui/button"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "./ui/tooltip"
import { FolderOpenIcon } from "lucide-react"

type RawFileCardProps = {
  selectedFilePath: string | null
  selectedFileReady: boolean
  readyActionLoading: boolean
  onToggleReady: () => void
  selectedFileLoading: boolean
  selectedFileError: string | null
  selectedFileContent: string
  onOpenFile: (app: "default" | "obsidian") => Promise<void>
  className?: string
}

export function RawFileCard({
  selectedFilePath,
  selectedFileReady,
  readyActionLoading,
  onToggleReady,
  selectedFileLoading,
  selectedFileError,
  selectedFileContent,
  onOpenFile,
  className,
}: RawFileCardProps) {
  const [openError, setOpenError] = useState<string | null>(null)

  const handleOpen = (app: "default" | "obsidian") => {
    setOpenError(null)
    void onOpenFile(app).catch((error: unknown) => {
      setOpenError(error instanceof Error ? error.message : "Failed to open file")
    })
  }

  return (
    <section className={cn("overflow-hidden", className)}>
      <div className="border-b flex flex-col px-4 py-3 gap-2">
        <div className="flex items-center justify-between gap-2">
          <h2 className="text-sm font-semibold">Raw File Content</h2>
          <div className="flex items-center gap-2">
            {selectedFilePath && selectedFileReady ? (
              <span className="shrink-0 rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-semibold text-emerald-700">
                Ready
              </span>
            ) : null}
            <button
              type="button"
              onClick={onToggleReady}
              disabled={!selectedFilePath || readyActionLoading}
              className="rounded-md border px-3 py-1.5 text-xs font-medium disabled:cursor-not-allowed disabled:opacity-50 hover:bg-accent"
            >
              {readyActionLoading
                ? selectedFileReady
                  ? "Unreadying..."
                  : "Marking..."
                : selectedFileReady
                  ? "Unready"
                  : "Mark Ready"}
            </button>
          </div>
        </div>
        <p className="mt-1 whitespace-normal break-all text-xs text-muted-foreground">
          {selectedFilePath ?? "No file selected"}
        </p>
        <div className="flex gap-2">
          <Tooltip>
            <TooltipTrigger >
              <Button
                onClick={() => handleOpen("default")}
                variant="outline"
                disabled={!selectedFilePath}
              >
                <FolderOpenIcon />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Open File</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  size="default"
                  variant="outline"
                  aria-label="Open with Obsidian"
                  onClick={() => handleOpen("obsidian")}
                  disabled={!selectedFilePath}
                />
              }
            >
              <img
                src="/obsidian-icon.svg"
                alt=""
                className="size-4"
              />
            </TooltipTrigger>
            <TooltipContent>Open with Obsidian</TooltipContent>
          </Tooltip>
        </div>
        {openError ? <p className="text-xs text-destructive">{openError}</p> : null}
      </div>
      <div className="p-0">
        {selectedFileLoading ? (
          <p className="text-sm text-muted-foreground">Loading file…</p>
        ) : selectedFileError ? (
          <p className="text-sm text-destructive">{selectedFileError}</p>
        ) : selectedFilePath ? (
          <pre className="whitespace-pre-wrap break-words bg-muted p-3 text-sm leading-relaxed">
            {selectedFileContent}
          </pre>
        ) : (
          <p className="text-sm text-muted-foreground">Select a markdown file in Catalog.</p>
        )}
      </div>
    </section>
  )
}
