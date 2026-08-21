type RawFileCardProps = {
  selectedFilePath: string | null
  selectedFileLoading: boolean
  selectedFileError: string | null
  selectedFileContent: string
  className?: string
}

export function RawFileCard({
  selectedFilePath,
  selectedFileLoading,
  selectedFileError,
  selectedFileContent,
  className,
}: RawFileCardProps) {
  return (
    <section className={className}>
      <div className="border-b px-4 py-3">
        <h2 className="text-sm font-semibold">Raw File Content</h2>
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
