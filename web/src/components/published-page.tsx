import { useEffect, useMemo, useState } from "react"
import { Link2Icon } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { fileNameFromPath } from "@/lib/catalogPath"
import { listJobAttempts, listPublishedJobs, type JobItem } from "@/lib/jobsApi"
import { useCatalogStore } from "@/store/catalogStore"
import { useUiStore } from "@/store/uiStore"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"

export function PublishedPage() {
  const [items, setItems] = useState<JobItem[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const selectFile = useCatalogStore((state) => state.selectFile)
  const loadCatalog = useCatalogStore((state) => state.loadCatalog)
  const revealFileInTree = useCatalogStore((state) => state.revealFileInTree)
  const setSelectedAttempts = useCatalogStore((state) => state.setSelectedAttempts)
  const setSelectedPublication = useCatalogStore(
    (state) => state.setSelectedPublication
  )
  const setActivePage = useUiStore((state) => state.setActivePage)
  const setPreviewPanelOpen = useUiStore((state) => state.setCatalogPanelOpen)

  const load = async () => {
    setLoading(true)
    setError(null)
    try {
      setItems(await listPublishedJobs())
    } catch (err) {
      setError(err instanceof Error ? err.message : "failed to load published jobs")
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [])

  const sortedItems = useMemo(
    () =>
      [...items].sort((a, b) =>
        (b.published_at ?? b.updated_at).localeCompare(
          a.published_at ?? a.updated_at
        )
      ),
    [items]
  )

  const selectRow = async (job: JobItem) => {
    setSelectedId(job.id)
    setPreviewPanelOpen(true)
    setError(null)
    try {
      await selectFile(job.file_path)
      const attempts = await listJobAttempts(job.id)
      setSelectedAttempts(attempts)
      setSelectedPublication({
        imported: isImportedPublication(job) && attempts.length === 0,
        publishedAt: job.published_at,
        operator: job.operator,
        note: job.ai_note ?? job.user_note,
      })
    } catch (err) {
      setError(err instanceof Error ? err.message : "failed to load publish details")
    }
  }

  const showFile = async (job: JobItem) => {
    setError(null)
    try {
      setActivePage("catalog")
      await loadCatalog()
      revealFileInTree(job.file_path)
      await selectFile(job.file_path)
    } catch (err) {
      setError(err instanceof Error ? err.message : "failed to open file in catalog")
    }
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto p-4">
      <div className="rounded-xl border bg-card p-6">
        <div className="mb-4 flex items-center justify-between gap-3">
          <div>
            <h2 className="text-xl font-semibold">Published</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              Completed publishing history.
            </p>
          </div>
          <Button variant="outline" disabled={loading} onClick={() => void load()}>
            Refresh
          </Button>
        </div>

        {error ? <p className="mb-3 text-sm text-destructive">{error}</p> : null}

        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="min-w-[15rem]">File</TableHead>
              <TableHead>Platform</TableHead>
              <TableHead>Published</TableHead>
              <TableHead>Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {sortedItems.length === 0 ? (
              <TableRow>
                <TableCell colSpan={4} className="text-sm text-muted-foreground">
                  {loading ? "Loading published jobs..." : "No published jobs yet."}
                </TableCell>
              </TableRow>
            ) : (
              sortedItems.map((job) => (
                <TableRow
                  key={job.id}
                  className={selectedRowClass(selectedId === job.id)}
                  onClick={() => {
                    void selectRow(job)
                  }}
                >
                  <TableCell className="min-w-[15rem] align-top">
                    <div className="flex flex-col gap-1">
                      <span className="whitespace-normal break-words">
                        {fileNameFromPath(job.file_path)}
                      </span>
                      <Badge variant="outline" className="w-fit">
                        {job.id.slice(0, 8)}
                      </Badge>
                      {isImportedPublication(job) ? (
                        <Badge variant="secondary" className="w-fit">
                          Imported
                        </Badge>
                      ) : null}
                    </div>
                  </TableCell>
                  <TableCell>
                    <Badge variant="secondary">{job.platform ?? "none"}</Badge>
                  </TableCell>
                  <TableCell>
                    {job.published_at
                      ? formatPublishedAt(job.published_at)
                      : "Date unknown"}
                  </TableCell>
                  <TableCell>
                    <div onClick={(event) => event.stopPropagation()}>
                      <Tooltip>
                        <TooltipTrigger
                          render={
                            <Button
                              size="icon-sm"
                              variant="outline"
                              onClick={() => {
                                void showFile(job)
                              }}
                            />
                          }
                        >
                          <Link2Icon className="size-4" />
                        </TooltipTrigger>
                        <TooltipContent>show the file</TooltipContent>
                      </Tooltip>
                    </div>
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>
    </div>
  )
}

function formatPublishedAt(value: string) {
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString()
}

function isImportedPublication(job: JobItem): boolean {
  return (
    job.attempt_count === 0 &&
    job.status_reason === "Imported historical publication"
  )
}

function selectedRowClass(selected: boolean): string {
  if (!selected) return "cursor-pointer hover:bg-muted/40"

  return [
    "cursor-pointer bg-blue-100/70 ring-1 ring-inset ring-blue-300",
    "hover:bg-blue-100/70 dark:bg-blue-950/40 dark:ring-blue-700",
  ].join(" ")
}
