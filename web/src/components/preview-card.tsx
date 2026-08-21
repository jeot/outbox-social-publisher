import type { CSSProperties } from "react"
import type { MediaPreview } from "@/store/catalogStore"

type PreviewCardProps = {
  selectedFilePath: string | null
  selectedPreviewPublishable: boolean
  selectedFileLoading: boolean
  selectedFileError: string | null
  selectedPublishText: string
  selectedPreviewMedia: MediaPreview[]
  selectedPreviewIssues: string[]
  className?: string
  style?: CSSProperties
}

export function PreviewCard({
  selectedFilePath,
  selectedPreviewPublishable,
  selectedFileLoading,
  selectedFileError,
  selectedPublishText,
  selectedPreviewMedia,
  selectedPreviewIssues,
  className,
  style,
}: PreviewCardProps) {
  return (
    <section className={className} style={style}>
      <div className="border-b px-4 py-3">
        <h2 className="text-sm font-semibold">Publish Preview</h2>
        <p className="mt-1 text-xs text-muted-foreground">
          {selectedFilePath
            ? selectedPreviewPublishable
              ? "Publishable"
              : "Blocked by validation issues"
            : "No file selected"}
        </p>
      </div>
      <div className="space-y-4 p-4">
        {selectedFileLoading ? (
          <p className="text-sm text-muted-foreground">Loading preview…</p>
        ) : selectedFileError ? (
          <p className="text-sm text-destructive">{selectedFileError}</p>
        ) : selectedFilePath ? (
          <>
            <div>
              <p className="mb-2 text-xs font-medium text-muted-foreground">
                Publish Text
              </p>
              <pre className="whitespace-pre-wrap break-words bg-muted p-3 text-sm leading-relaxed">
                {selectedPublishText || "(empty)"}
              </pre>
            </div>

            <div>
              <p className="mb-2 text-xs font-medium text-muted-foreground">Media</p>
              {selectedPreviewMedia.length === 0 ? (
                <p className="text-sm text-muted-foreground">No media detected.</p>
              ) : (
                <ul className="space-y-2">
                  {selectedPreviewMedia.map((item, idx) => (
                    <li
                      key={`${item.reference}-${idx}`}
                      className="rounded-md border bg-muted/40 p-2 text-xs"
                    >
                      <p className="break-all">{item.reference}</p>
                      {item.resolved_path ? (
                        <p className="mt-1 break-all text-muted-foreground">
                          {item.resolved_path}
                        </p>
                      ) : null}
                      {item.error ? (
                        <p className="mt-1 text-destructive">{item.error}</p>
                      ) : null}
                    </li>
                  ))}
                </ul>
              )}
            </div>

            <div>
              <p className="mb-2 text-xs font-medium text-muted-foreground">Issues</p>
              {selectedPreviewIssues.length === 0 ? (
                <p className="text-sm text-emerald-600">No issues.</p>
              ) : (
                <ul className="list-disc space-y-1 pl-5 text-sm text-destructive">
                  {selectedPreviewIssues.map((issue) => (
                    <li key={issue}>{issue}</li>
                  ))}
                </ul>
              )}
            </div>
          </>
        ) : (
          <p className="text-sm text-muted-foreground">
            Select a file from Catalog to preview publish content.
          </p>
        )}
      </div>
    </section>
  )
}
