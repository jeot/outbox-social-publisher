import { useEffect, useMemo, useState } from "react"
import { Link2Icon } from "lucide-react"
import { toast } from "sonner"

import {
  cancelJob,
  listScheduledJobs,
  listPublishingJobs,
  setScheduledJobTime,
  type JobItem,
} from "@/lib/jobsApi"
import { displayCatalogPath, fileNameFromPath } from "@/lib/catalogPath"
import { SCHEDULE_PRESETS, type SchedulePreset } from "@/lib/schedulePresets"
import { useCatalogStore } from "@/store/catalogStore"
import { useUiStore } from "@/store/uiStore"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { ScheduleControls } from "@/components/schedule-controls"
import { PublishingCard } from "@/components/publishing-card"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"

export function ScheduledPage() {
  const [scheduledItems, setScheduledItems] = useState<JobItem[]>([])
  const [publishingItems, setPublishingItems] = useState<JobItem[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [actionKey, setActionKey] = useState<string | null>(null)
  const [customDialogJobId, setCustomDialogJobId] = useState<string | null>(null)
  const [customDateById, setCustomDateById] = useState<Record<string, string>>({})
  const [customTimeById, setCustomTimeById] = useState<Record<string, string>>({})
  const [selectedRowId, setSelectedRowId] = useState<string | null>(null)

  const setActivePage = useUiStore((state) => state.setActivePage)
  const setPreviewPanelOpen = useUiStore((state) => state.setCatalogPanelOpen)
  const roots = useCatalogStore((state) => state.roots)
  const loadCatalog = useCatalogStore((state) => state.loadCatalog)
  const revealFileInTree = useCatalogStore((state) => state.revealFileInTree)
  const selectFile = useCatalogStore((state) => state.selectFile)
  const rootPaths = useMemo(() => roots.map((item) => item.root), [roots])

  const localTimeZone = useMemo(
    () => Intl.DateTimeFormat().resolvedOptions().timeZone ?? "Local",
    []
  )
  const sortedScheduledItems = useMemo(() => {
    const copy = [...scheduledItems]
    copy.sort((a, b) => {
      const aTime = utcMillis(a.run_at_utc)
      const bTime = utcMillis(b.run_at_utc)
      if (aTime === null && bTime === null) return 0
      if (aTime === null) return 1
      if (bTime === null) return -1
      return aTime - bTime
    })
    return copy
  }, [scheduledItems])

  const load = async () => {
    setLoading(true)
    setError(null)
    try {
      const [scheduled, publishing] = await Promise.all([
        listScheduledJobs(),
        listPublishingJobs(),
      ])
      setScheduledItems(scheduled)
      setPublishingItems(publishing)
    } catch (err) {
      setError(err instanceof Error ? err.message : "failed to load jobs")
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [])

  const runAction = async (key: string, fn: () => Promise<void>) => {
    setActionKey(key)
    setError(null)
    try {
      await fn()
      await Promise.all([load(), loadCatalog()])
    } catch (err) {
      setError(err instanceof Error ? err.message : "action failed")
    } finally {
      setActionKey(null)
    }
  }

  const showFile = async (path: string) => {
    setError(null)
    try {
      setActivePage("catalog")
      await loadCatalog()
      revealFileInTree(path)
      await selectFile(path)
    } catch (err) {
      setError(err instanceof Error ? err.message : "failed to open file in catalog")
    }
  }

  const selectRowForPreview = async (job: JobItem) => {
    setSelectedRowId(job.id)
    setPreviewPanelOpen(true)
    try {
      await selectFile(job.file_path)
    } catch (err) {
      setError(err instanceof Error ? err.message : "failed to load preview")
    }
  }

  const openCustomForm = (job: JobItem) => {
    const initial = job.run_at_utc
      ? toLocalDateTimeParts(new Date(job.run_at_utc))
      : toLocalDateTimeParts(new Date())
    setCustomDateById((state) => ({ ...state, [job.id]: initial.date }))
    setCustomTimeById((state) => ({ ...state, [job.id]: initial.time }))
    setCustomDialogJobId(job.id)
  }

  const saveScheduledTime = async (jobId: string) => {
    const datePart = customDateById[jobId]
    const timePart = customTimeById[jobId]
    if (!datePart || !timePart) {
      setError("Select date and time first.")
      return
    }
    const date = new Date(`${datePart}T${timePart}`)
    if (Number.isNaN(date.getTime())) {
      setError("Invalid datetime.")
      return
    }
    await runAction(`reschedule:${jobId}`, async () => {
      await setScheduledJobTime(jobId, date.toISOString(), localTimeZone)
    })
    setCustomDialogJobId(null)
  }

  const rescheduleWithPreset = async (job: JobItem, preset: SchedulePreset) => {
    const at = preset.at().toISOString()
    await runAction(`reschedule:${job.id}:${preset.key}`, async () => {
      await setScheduledJobTime(job.id, at, localTimeZone)
    })
  }

  const customDialogJob = customDialogJobId
    ? scheduledItems.find((item) => item.id === customDialogJobId) ?? null
    : null
  const customDialogDate = customDialogJobId ? customDateById[customDialogJobId] ?? "" : ""
  const customDialogTime = customDialogJobId ? customTimeById[customDialogJobId] ?? "" : ""
  const customDialogBusy = customDialogJobId
    ? Boolean(actionKey?.includes(customDialogJobId))
    : false

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-4">
      <div className="rounded-xl border bg-card p-6">
        <div className="mb-3 flex items-center justify-between gap-2">
          <div>
            <h2 className="text-xl font-semibold">Scheduled</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              Review scheduled jobs and cancel as needed.
            </p>
          </div>
          <Button variant="outline" onClick={() => void load()} disabled={loading}>
            Refresh
          </Button>
        </div>

        {error ? <p className="mb-3 text-sm text-destructive">{error}</p> : null}

        <PublishingCard
          items={publishingItems}
          onShowFile={(job) => {
            void showFile(job.file_path)
          }}
        />

        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="min-w-[15rem]">File</TableHead>
              <TableHead>Platform</TableHead>
              <TableHead>{`Schedule (${localTimeZone})`}</TableHead>
              <TableHead>Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {sortedScheduledItems.length === 0 ? (
              <TableRow>
                <TableCell colSpan={4} className="text-sm text-muted-foreground">
                  {loading ? "Loading scheduled jobs..." : "No scheduled jobs yet."}
                </TableCell>
              </TableRow>
            ) : (
              sortedScheduledItems.map((job) => {
                const running = actionKey?.endsWith(job.id)
                const fileName = fileNameFromPath(job.file_path)

                return (
                  <TableRow
                    key={job.id}
                    className={selectedRowClass(selectedRowId === job.id)}
                    onClick={() => {
                      void selectRowForPreview(job)
                    }}
                  >
                    <TableCell className="min-w-[15rem] max-w-[32rem] align-top">
                      <div className="flex flex-col gap-1">
                        <span className="whitespace-normal break-words">{fileName}</span>
                        <Badge variant="outline">{job.id.slice(0, 8)}</Badge>
                      </div>
                    </TableCell>
                    <TableCell>
                      <Badge variant="secondary">{job.platform ?? "none"}</Badge>
                    </TableCell>
                    <TableCell>
                      <ScheduleControls
                        presets={SCHEDULE_PRESETS}
                        disabled={Boolean(running)}
                        customLabel={formatRunAtLocal(job.run_at_utc)}
                        showPastWarning={isPastRunAt(job.run_at_utc)}
                        showAiIcon={job.operator === "ai"}
                        onPresetSelect={(preset) => {
                          void rescheduleWithPreset(job, preset)
                        }}
                        onCustomClick={() => openCustomForm(job)}
                      />
                    </TableCell>
                    <TableCell>
                      <div
                        className="flex items-center gap-2"
                        onClick={(event) => event.stopPropagation()}
                      >
                        <Tooltip>
                          <TooltipTrigger
                            render={
                              <Button
                                size="icon-sm"
                                variant="outline"
                                disabled={Boolean(running)}
                                onClick={() => {
                                  void showFile(job.file_path)
                                }}
                              />
                            }
                          >
                            <Link2Icon className="size-4" />
                          </TooltipTrigger>
                          <TooltipContent>show the file</TooltipContent>
                        </Tooltip>
                        <Button
                          size="sm"
                          variant="destructive"
                          disabled={Boolean(running)}
                          onClick={() => {
                            const confirmed = window.confirm(
                              "Cancel this scheduled job and move it to Decision Queue?"
                            )
                            if (!confirmed) return
                            void runAction(`cancel:${job.id}`, async () => {
                              await cancelJob(job.id)
                              toast.info(
                                "The scheduled item was canceled. You can review it " +
                                  'in the "Decision Queue" page.',
                                { duration: 30_000 }
                              )
                            })
                          }}
                        >
                          Cancel
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                )
              })
            )}
          </TableBody>
        </Table>
      </div>
      <Dialog
        open={customDialogJobId !== null}
        onOpenChange={(open) => {
          if (!open) setCustomDialogJobId(null)
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Reschedule</DialogTitle>
            <DialogDescription>
              {customDialogJob
                ? displayCatalogPath(customDialogJob.file_path, rootPaths)
                : "Select new date and time."}
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-3">
            <div className="grid gap-1">
              <Label htmlFor="scheduled-custom-date">Date</Label>
              <Input
                id="scheduled-custom-date"
                type="date"
                value={customDialogDate}
                onChange={(event) => {
                  if (!customDialogJobId) return
                  setCustomDateById((state) => ({
                    ...state,
                    [customDialogJobId]: event.target.value,
                  }))
                }}
              />
            </div>
            <div className="grid gap-1">
              <Label htmlFor="scheduled-custom-time">Time</Label>
              <Input
                id="scheduled-custom-time"
                type="time"
                value={customDialogTime}
                onChange={(event) => {
                  if (!customDialogJobId) return
                  setCustomTimeById((state) => ({
                    ...state,
                    [customDialogJobId]: event.target.value,
                  }))
                }}
              />
            </div>
          </div>
          <DialogFooter>
            <Button
              disabled={!customDialogJobId || customDialogBusy}
              onClick={() => {
                if (!customDialogJobId) return
                void saveScheduledTime(customDialogJobId)
              }}
            >
              Apply
            </Button>
            <Button
              variant="outline"
              disabled={customDialogBusy}
              onClick={() => setCustomDialogJobId(null)}
            >
              Cancel
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}

function utcMillis(raw: string | null): number | null {
  if (!raw) return null
  const value = Date.parse(raw)
  return Number.isNaN(value) ? null : value
}

function selectedRowClass(selected: boolean): string {
  if (!selected) return "cursor-pointer hover:bg-muted/40"

  return [
    "cursor-pointer bg-blue-100/70 ring-1 ring-inset ring-blue-300",
    "hover:bg-blue-100/70 dark:bg-blue-950/40 dark:ring-blue-700",
  ].join(" ")
}

function formatRunAtLocal(rawUtc: string | null): string {
  if (!rawUtc) return "-"
  const date = new Date(rawUtc)
  if (Number.isNaN(date.getTime())) return rawUtc

  const nowYear = new Date().getFullYear()
  const includeYear = date.getFullYear() !== nowYear
  const datePart = new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "2-digit",
    ...(includeYear ? { year: "numeric" } : {}),
  }).format(date)
  const timePart = new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    hour12: true,
  }).format(date)
  const weekdayPart = new Intl.DateTimeFormat(undefined, {
    weekday: "short",
  }).format(date)

  return `${datePart}, ${timePart} (${weekdayPart})`
}

function isPastRunAt(rawUtc: string | null): boolean {
  if (!rawUtc) return false
  const time = Date.parse(rawUtc)
  if (Number.isNaN(time)) return false
  return time < Date.now()
}

function toDateTimeLocalValue(date: Date): string {
  const offsetMs = date.getTimezoneOffset() * 60_000
  const local = new Date(date.getTime() - offsetMs)
  return local.toISOString().slice(0, 16)
}

function toLocalDateTimeParts(date: Date): { date: string; time: string } {
  const local = toDateTimeLocalValue(date)
  const [datePart, timePart] = local.split("T")
  return {
    date: datePart ?? "",
    time: timePart ?? "09:00",
  }
}
