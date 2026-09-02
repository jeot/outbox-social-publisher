import type { CSSProperties } from "react"
import { ExternalLink, LoaderCircle } from "lucide-react"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import type { LinkPreview, MediaPreview } from "@/store/catalogStore"

type PreviewCardProps = {
  selectedFilePath: string | null
  selectedFileName: string
  selectedFileDisplayPath: string
  selectedPreviewPublishable: boolean
  selectedFileLoading: boolean
  selectedFileError: string | null
  selectedPublishText: string
  selectedPreviewMedia: MediaPreview[]
  selectedPreviewLink: LinkPreview | null
  selectedPreviewLinkLoading: boolean
  selectedPreviewIssues: string[]
  className?: string
  style?: CSSProperties
}

export function PreviewCard({
  selectedFilePath,
  selectedFileName,
  selectedFileDisplayPath,
  selectedPreviewPublishable,
  selectedFileLoading,
  selectedFileError,
  selectedPublishText,
  selectedPreviewMedia,
  selectedPreviewLink,
  selectedPreviewLinkLoading,
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
                {selectedPreviewLink?.status === "found" && selectedPreviewLink.url ? (
                  <Card size="sm" className="border-primary/30 bg-primary/5">
                    {selectedPreviewLink.thumbnail_url ? (
                      <img
                        src={selectedPreviewLink.thumbnail_url}
                        alt=""
                        className="aspect-[1.91/1] w-full object-cover"
                        loading="lazy"
                      />
                    ) : null}
                    <CardHeader>
                      <CardTitle className="flex items-center gap-2 text-sm">
                        <ExternalLink className="size-4" />
                        Link preview
                      </CardTitle>
                    </CardHeader>
                    <CardContent className="space-y-1">
                      {selectedPreviewLinkLoading ? (
                        <div className="flex items-center gap-2 text-xs text-muted-foreground">
                          <LoaderCircle className="size-3.5 animate-spin" />
                          Loading link preview…
                        </div>
                      ) : null}
                      {selectedPreviewLink.title ? (
                        <p className="text-sm font-semibold">
                          {selectedPreviewLink.title}
                        </p>
                      ) : null}
                      {selectedPreviewLink.description ? (
                        <p className="line-clamp-3 text-xs text-muted-foreground">
                          {selectedPreviewLink.description}
                        </p>
                      ) : null}
                      <p className="text-xs font-medium text-muted-foreground">
                        {selectedPreviewLink.domain ?? "External link"}
                      </p>
                      <a
                        href={selectedPreviewLink.url}
                        target="_blank"
                        rel="noreferrer"
                        className="block break-all text-sm text-primary underline-offset-4 hover:underline"
                      >
                        {selectedPreviewLink.url}
                      </a>
                      <p className="text-xs text-muted-foreground">
                        Attached to Substack Notes and to LinkedIn posts without native images.
                      </p>
                      {selectedPreviewLink.metadata_error ? (
                        <p className="text-xs text-amber-700 dark:text-amber-400">
                          Metadata unavailable; the URL was still detected.
                        </p>
                      ) : null}
                    </CardContent>
                  </Card>
                ) : null}
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

            <div className="space-y-2 rounded-md border bg-muted/20 p-2 g-2 text-xs">
              <div>
                <p className="font-bold text-muted-foreground">File Name</p>
                <p className="text-sm text-muted-foreground break-all">{selectedFileName || "-"}</p>
              </div>
              <div>
                <p className="font-bold text-muted-foreground">File Path</p>
                <p className="text-sm text-muted-foreground break-all">{selectedFileDisplayPath || "-"}</p>
              </div>
              <div>
                <p className="font-bold text-muted-foreground">Media</p>
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
                <p className="font-bold text-muted-foreground">Issues</p>
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
            </div>
          </>
        ) : (
          <p className="text-sm text-muted-foreground">
            Select an item to preview.
          </p>
        )}
      </div>
    </section>
  )
}
