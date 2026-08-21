import { cn } from "@/lib/utils"

type RawFileCardProps = {
  selectedFilePath: string | null
  selectedFileReady: boolean
  selectedFileLoading: boolean
  selectedFileError: string | null
  selectedFileContent: string
  className?: string
}

export function RawFileCard({
  selectedFilePath,
  selectedFileReady,
  selectedFileLoading,
  selectedFileError,
  selectedFileContent,
  className,
}: RawFileCardProps) {
  return (
    <section className={cn("overflow-hidden", className)}>
      <div className="border-b px-4 py-3">
        <div className="flex items-center justify-between gap-2">
          <h2 className="text-sm font-semibold">Raw File Content</h2>
          {selectedFilePath && selectedFileReady ? (
            <span className="shrink-0 rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-semibold text-emerald-700">
              Ready
            </span>
          ) : null}
        </div>
        <p className="mt-1 whitespace-normal break-all text-xs text-muted-foreground">
          {selectedFilePath ?? "No file selected"}
        </p>
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
