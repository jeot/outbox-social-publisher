import type { CSSProperties } from "react"
import type { MediaPreview } from "@/store/catalogStore"

type PreviewCardProps = {
  selectedFilePath: string | null
  selectedPreviewPublishable: boolean
  selectedFileReady: boolean
  readyActionLoading: boolean
  onToggleReady: () => void
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
  selectedFileReady,
  readyActionLoading,
  onToggleReady,
  selectedFileLoading,
  selectedFileError,
  selectedPublishText,
  selectedPreviewMedia,
  selectedPreviewIssues,
  className,
  style,
}: PreviewCardProps) {
  const previewImages = selectedPreviewMedia.filter(
    (item) =>
      item.exists &&
      item.valid_extension &&
      !item.error &&
      typeof item.resolved_path === "string" &&
      item.resolved_path.length > 0
  )

  return (
    <section className={className} style={style}>
      <div className="flex items-center justify-between border-b px-4 py-3">
        <div>
          <h2 className="text-sm font-semibold">Publish Preview</h2>
          <p className="mt-1 text-xs text-muted-foreground">
            {selectedFilePath
              ? selectedPreviewPublishable
                ? "Publishable"
                : "Blocked by validation issues"
              : "No file selected"}
          </p>
        </div>
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
      <div className="space-y-4 p-4">
        {selectedFileLoading ? (
          <p className="text-sm text-muted-foreground">Loading preview…</p>
        ) : selectedFileError ? (
          <p className="text-sm text-destructive">{selectedFileError}</p>
        ) : selectedFilePath ? (
          <>
            <div className="overflow-hidden rounded-xl bg-card">
              <div className="space-y-3 p-4">
                <p className="whitespace-pre-wrap break-words text-md leading-relaxed">
                  {selectedPublishText || "(empty)"}
                </p>
                {previewImages.length > 0 ? (
                  <div className="space-y-2">
                    {previewImages.map((item, idx) => (
                      <img
                        key={`${item.reference}-${idx}-img`}
                        src={`/api/catalog/media?path=${encodeURIComponent(item.resolved_path!)}`}
                        alt={item.reference}
                        className="w-full rounded-lg border object-cover"
                        loading="lazy"
                      />
                    ))}
                  </div>
                ) : null}
              </div>
            </div>

            <div className="border border-border">
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

            <div className="p4">
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
