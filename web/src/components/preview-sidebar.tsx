import { PreviewCard } from "@/components/preview-card"
import { FailedJobDetailsCard } from "@/components/failed-job-details-card"
import { PublicationTimeline } from "@/components/publication-timeline"
import { fileNameFromPath, tooltipCatalogPath } from "@/lib/catalogPath"
import { useCatalogStore } from "@/store/catalogStore"

export function PreviewSidebar() {
  const selectedFilePath = useCatalogStore((state) => state.selectedFilePath)
  const roots = useCatalogStore((state) => state.roots)
  const selectedPublishText = useCatalogStore((state) => state.selectedPublishText)
  const selectedPreviewMedia = useCatalogStore((state) => state.selectedPreviewMedia)
  const selectedPreviewLink = useCatalogStore((state) => state.selectedPreviewLink)
  const selectedPreviewLinkLoading = useCatalogStore(
    (state) => state.selectedPreviewLinkLoading
  )
  const selectedPreviewIssues = useCatalogStore((state) => state.selectedPreviewIssues)
  const selectedPreviewPublishable = useCatalogStore(
    (state) => state.selectedPreviewPublishable
  )
  const selectedFileLoading = useCatalogStore((state) => state.selectedFileLoading)
  const selectedFileError = useCatalogStore((state) => state.selectedFileError)
  const selectedAttempts = useCatalogStore((state) => state.selectedAttempts)
  const selectedPublication = useCatalogStore(
    (state) => state.selectedPublication
  )
  const selectedFailedJob = useCatalogStore((state) => state.selectedFailedJob)
  const rootPaths = roots.map((item) => item.root)
  const selectedFileName = fileNameFromPath(selectedFilePath)
  const selectedFileDisplayPath = selectedFilePath
    ? tooltipCatalogPath(selectedFilePath, rootPaths)
    : ""

  return (
    <div className="h-full min-h-0 overflow-y-auto p-4">
      {!selectedFailedJob ? (
        <PublicationTimeline
          attempts={selectedAttempts}
          publication={selectedPublication}
        />
      ) : null}
      <PreviewCard
        selectedFilePath={selectedFilePath}
        selectedFileName={selectedFileName}
        selectedFileDisplayPath={selectedFileDisplayPath}
        selectedPreviewPublishable={selectedPreviewPublishable}
        selectedFileLoading={selectedFileLoading}
        selectedFileError={selectedFileError}
        selectedPublishText={selectedPublishText}
        selectedPreviewMedia={selectedPreviewMedia}
        selectedPreviewLink={selectedPreviewLink}
        selectedPreviewLinkLoading={selectedPreviewLinkLoading}
        selectedPreviewIssues={selectedPreviewIssues}
        className="flex min-h-0 flex-col rounded-xl border bg-card"
      />
      {selectedFailedJob ? <FailedJobDetailsCard job={selectedFailedJob} /> : null}
    </div>
  )
}
