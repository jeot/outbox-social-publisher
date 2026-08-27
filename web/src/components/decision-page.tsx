import { useEffect, useMemo, useState } from "react"
import { CheckIcon, Link2Icon, SparklesIcon } from "lucide-react"

import {
  listBlockedJobs,
  listCanceledJobs,
  listDisabledJobs,
  listFailedJobs,
  listReadyJobs,
  scheduleJobMulti,
  setReadyJobTime,
  setReadyJobPlatforms,
  type JobItem,
  unreadyJob,
} from "@/lib/jobsApi"
import { displayCatalogPath, fileNameFromPath } from "@/lib/catalogPath"
import { SCHEDULE_PRESETS, type SchedulePreset } from "@/lib/schedulePresets"
import { useCatalogStore } from "@/store/catalogStore"
import { useUiStore } from "@/store/uiStore"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { FailedJobsTable } from "@/components/failed-jobs-table"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { ScheduleControls } from "@/components/schedule-controls"
import { StatusBadge } from "@/components/status-badge"
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

type PlatformSelection = {
  linkedin: boolean
  x: boolean
}

export function DecisionPage() {
  const [readyItems, setReadyItems] = useState<JobItem[]>([])
  const [blockedItems, setBlockedItems] = useState<JobItem[]>([])
  const [canceledItems, setCanceledItems] = useState<JobItem[]>([])
  const [disabledItems, setDisabledItems] = useState<JobItem[]>([])
  const [failedItems, setFailedItems] = useState<JobItem[]>([])
  const [readyHasIssueById, setReadyHasIssueById] = useState<Record<string, boolean>>({})
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [actionKey, setActionKey] = useState<string | null>(null)
  const [savingPlatformsById, setSavingPlatformsById] = useState<Record<string, boolean>>({})
  const [customDateById, setCustomDateById] = useState<Record<string, string>>({})
  const [customTimeById, setCustomTimeById] = useState<Record<string, string>>({})
  const [customDialogJobId, setCustomDialogJobId] = useState<string | null>(null)
  const [platformById, setPlatformById] = useState<Record<string, PlatformSelection>>({})
  const [selectedRowId, setSelectedRowId] = useState<string | null>(null)
  const setActivePage = useUiStore((state) => state.setActivePage)
  const setPreviewPanelOpen = useUiStore((state) => state.setCatalogPanelOpen)
  const roots = useCatalogStore((state) => state.roots)
  const loadCatalog = useCatalogStore((state) => state.loadCatalog)
  const revealFileInTree = useCatalogStore((state) => state.revealFileInTree)
  const selectFile = useCatalogStore((state) => state.selectFile)
  const rootPaths = useMemo(() => roots.map((item) => item.root), [roots])
  const staleItems = useMemo(
    () => [...blockedItems, ...canceledItems, ...disabledItems],
    [blockedItems, canceledItems, disabledItems]
  )
  const decisionItems = useMemo(() => [...readyItems, ...staleItems], [readyItems, staleItems])

  const timezone = useMemo(
    () => Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC",
    []
  )

  const load = async () => {
    setLoading(true)
    setError(null)
    try {
      const [ready, blocked, canceled, disabled, failed] = await Promise.all([
        listReadyJobs(),
        listBlockedJobs(),
        listCanceledJobs(),
        listDisabledJobs(),
        listFailedJobs(),
      ])
      setReadyItems(ready)
      setBlockedItems(blocked)
      setCanceledItems(canceled)
      setDisabledItems(disabled)
      setFailedItems(failed)
      const issueChecks = await Promise.all(
        ready.map(async (job) => ({
          id: job.id,
          hasIssue: await checkReadyFileHasIssue(job.file_path),
        }))
      )
      const nextIssueMap: Record<string, boolean> = {}
      for (const item of issueChecks) nextIssueMap[item.id] = item.hasIssue
      setReadyHasIssueById(nextIssueMap)
      setPlatformById((state) => {
        const updated = { ...state }
        const decision = [...ready, ...blocked, ...canceled, ...disabled, ...failed]
        for (const job of decision) {
          const selected = Array.isArray(job.selected_platforms)
            ? job.selected_platforms
            : []
          updated[job.id] = {
            linkedin: selected.includes("linkedin") || job.platform === "linkedin",
            x: selected.includes("x") || job.platform === "x",
          }
        }
        return updated
      })
    } catch (err) {
      setError(err instanceof Error ? err.message : "failed to load ready jobs")
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

  const selectedPlatforms = (jobId: string): Array<"linkedin" | "x"> => {
    const existing = platformById[jobId]
    const fallbackJob = decisionItems.find((item) => item.id === jobId)
    const fallbackSelected = Array.isArray(fallbackJob?.selected_platforms)
      ? fallbackJob.selected_platforms
      : []
    const selection: PlatformSelection = existing ?? {
      linkedin:
        fallbackSelected.includes("linkedin") || fallbackJob?.platform === "linkedin",
      x: fallbackSelected.includes("x") || fallbackJob?.platform === "x",
    }
    const out: Array<"linkedin" | "x"> = []
    if (selection.linkedin) out.push("linkedin")
    if (selection.x) out.push("x")
    return out
  }

  const persistPlatforms = async (jobId: string, selection: PlatformSelection) => {
    const selected = selectedPlatformsFromSelection(selection)
    setSavingPlatformsById((state) => ({ ...state, [jobId]: true }))
    try {
      await setReadyJobPlatforms(jobId, selected)
    } catch (err) {
      setError(err instanceof Error ? err.message : "failed to save platform selection")
      await load()
    } finally {
      setSavingPlatformsById((state) => ({ ...state, [jobId]: false }))
    }
  }

  const scheduleWithPreset = async (job: JobItem, preset: SchedulePreset) => {
    const platforms = selectedPlatforms(job.id)
    if (platforms.length === 0) {
      setError("Select at least one platform before scheduling.")
      return
    }

    const at = preset.at().toISOString()
    await runAction(`schedule:${job.id}:${preset.key}`, async () => {
      await scheduleJobMulti(job.id, at, timezone, platforms)
    })
  }

  const scheduleCustom = async (jobId: string) => {
    const datePart = customDateById[jobId]
    const timePart = customTimeById[jobId]
    if (!datePart || !timePart) {
      setError("Select custom date and time first.")
      return
    }

    const date = new Date(`${datePart}T${timePart}`)
    if (Number.isNaN(date.getTime())) {
      setError("Invalid custom datetime.")
      return
    }

    await runAction(`ready-time:${jobId}`, async () => {
      await setReadyJobTime(jobId, date.toISOString(), timezone)
    })
    setCustomDialogJobId(null)
  }

  const applySuggestedSchedule = async (job: JobItem) => {
    if (!job.run_at_utc) {
      setError("Set a schedule time first.")
      return
    }
    const platforms = selectedPlatforms(job.id)
    if (platforms.length === 0) {
      setError("Select at least one platform before scheduling.")
      return
    }
    await runAction(`schedule-suggested:${job.id}`, async () => {
      await scheduleJobMulti(job.id, job.run_at_utc!, timezone, platforms)
    })
  }

  const openCustomForm = (jobId: string) => {
    const current = decisionItems.find((item) => item.id === jobId)
    const initial = current?.run_at_utc
      ? toLocalDateTimeParts(new Date(current.run_at_utc))
      : toLocalDateTimeParts(new Date())
    setCustomDateById((state) =>
      ({ ...state, [jobId]: initial.date })
    )
    setCustomTimeById((state) =>
      ({ ...state, [jobId]: initial.time })
    )
    setCustomDialogJobId(jobId)
  }

  const customDialogJob = customDialogJobId
    ? decisionItems.find((item) => item.id === customDialogJobId) ?? null
    : null
  const customDialogDate = customDialogJobId ? customDateById[customDialogJobId] ?? "" : ""
  const customDialogTime = customDialogJobId ? customTimeById[customDialogJobId] ?? "" : ""
  const customDialogBusy = customDialogJobId
    ? Boolean(actionKey?.includes(customDialogJobId)) ||
    Boolean(savingPlatformsById[customDialogJobId])
    : false

  return (
    <>
      <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-4">
        <div className="rounded-xl border bg-card p-6">
          <div className="mb-3 flex items-center justify-between gap-2">
            <div>
              <h2 className="text-xl font-semibold">Decision Queue</h2>
              <p className="mt-1 text-sm text-muted-foreground">
                Resolve everything that is not currently scheduled.
              </p>
            </div>
            <Button variant="outline" onClick={() => void load()} disabled={loading}>
              Refresh
            </Button>
          </div>

          {error ? <p className="mb-3 text-sm text-destructive">{error}</p> : null}

          <Table className="table-fixed">
            <TableHeader>
              <TableRow>
                <TableHead className="w-[15rem] min-w-[15rem]">File</TableHead>
                <TableHead className="w-[10rem]">Platforms</TableHead>
                <TableHead className="w-[18rem]">Schedule</TableHead>
                <TableHead className="w-[12rem]">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {decisionItems.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={4} className="text-sm text-muted-foreground">
                    {loading ? "Loading decision queue..." : "No items in decision queue."}
                  </TableCell>
                </TableRow>
              ) : (
                decisionItems
                  .sort(
                    (a, b) =>
                      decisionStatusRank(a.status) - decisionStatusRank(b.status)
                  )
                  .map((job) => {
                    const running = actionKey?.includes(job.id)
                    const savingPlatforms = Boolean(savingPlatformsById[job.id])
                    const isReady = job.status === "ready"
                    const fallbackSelected = Array.isArray(job.selected_platforms)
                      ? job.selected_platforms
                      : []
                    const fallbackSelection: PlatformSelection = {
                      linkedin:
                        fallbackSelected.includes("linkedin") ||
                        job.platform === "linkedin",
                      x: fallbackSelected.includes("x") || job.platform === "x",
                    }
                    const selection = platformById[job.id] ?? fallbackSelection
                    const fileName = fileNameFromPath(job.file_path)

                    return (
                      <TableRow
                        key={job.id}
                        className={selectedRowClass(selectedRowId === job.id)}
                        onClick={() => {
                          void selectRowForPreview(job)
                        }}
                      >
                        <TableCell className="w-[15rem] min-w-[15rem] align-top">
                          <div className="flex flex-col gap-1">
                            <span className="whitespace-normal break-words">{fileName}</span>
                            <div className="flex flex-wrap gap-2">
                              <StatusBadge status={job.status} />
                              <Badge variant="outline">{job.id.slice(0, 8)}</Badge>
                              {job.operator === "ai" ? (
                                <SparklesIcon className="size-4 text-emerald-500" />
                              ) : null}
                              {isReady && readyHasIssueById[job.id] ? (
                                <Badge variant="destructive">Has issue</Badge>
                              ) : job.status_reason ? (
                                <Badge variant="destructive">{job.status_reason}</Badge>
                              ) : null}
                            </div>
                          </div>
                        </TableCell>
                        <TableCell>
                          <div className="flex items-center gap-2">
                            <Button
                              size="sm"
                              variant={selection.linkedin ? "default" : "outline"}
                              className={
                                selection.linkedin
                                  ? "bg-emerald-600 text-white hover:bg-emerald-700"
                                  : ""
                              }
                              disabled={Boolean(running) || savingPlatforms}
                              onClick={() => {
                                const nextSelection = {
                                  linkedin: !selection.linkedin,
                                  x: selection.x,
                                }
                                setPlatformById((state) => ({
                                  ...state,
                                  [job.id]: nextSelection,
                                }))
                                void persistPlatforms(job.id, nextSelection)
                              }}
                            >
                              LinkedIn
                            </Button>
                            <Button
                              size="sm"
                              variant={selection.x ? "default" : "outline"}
                              className={
                                selection.x
                                  ? "bg-emerald-600 text-white hover:bg-emerald-700"
                                  : ""
                              }
                              disabled={Boolean(running) || savingPlatforms}
                              onClick={() => {
                                const nextSelection = {
                                  linkedin: selection.linkedin,
                                  x: !selection.x,
                                }
                                setPlatformById((state) => ({
                                  ...state,
                                  [job.id]: nextSelection,
                                }))
                                void persistPlatforms(job.id, nextSelection)
                              }}
                            >
                              X
                            </Button>
                          </div>
                        </TableCell>
                        <TableCell>
                          <ScheduleControls
                            presets={SCHEDULE_PRESETS}
                            disabled={Boolean(running) || savingPlatforms}
                            customLabel={readyScheduleLabel(job)}
                            showPastWarning={isPastRunAt(job.run_at_utc)}
                            showAiIcon={job.operator === "ai"}
                            onPresetSelect={(preset) => {
                              void scheduleWithPreset(job, preset)
                            }}
                            onCustomClick={() => openCustomForm(job.id)}
                          />
                        </TableCell>
                        <TableCell>
                          <div
                            className="flex items-center gap-2"
                            onClick={(event) => event.stopPropagation()}
                          >
                            {canScheduleFromSuggestion(job, selection) ? (
                              <Button
                                size="icon-sm"
                                variant="default"
                                disabled={Boolean(running) || savingPlatforms}
                                onClick={() => {
                                  void applySuggestedSchedule(job)
                                }}
                              >
                                <CheckIcon className="size-4" />
                              </Button>
                            ) : null}
                            <Tooltip>
                              <TooltipTrigger
                                render={
                                  <Button
                                    size="icon-sm"
                                    variant="outline"
                                    disabled={Boolean(running) || savingPlatforms}
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
                              variant="outline"
                              disabled={Boolean(running) || savingPlatforms}
                              onClick={() => {
                                if (
                                  !window.confirm(
                                    "Remove this item from decision queue?"
                                  )
                                ) {
                                  return
                                }
                                void runAction(`remove:${job.id}`, async () => {
                                  await unreadyJob(job.id)
                                })
                              }}
                            >
                              Remove
                            </Button>
                          </div>
                        </TableCell>
                      </TableRow>
                    )
                  })
              )}
            </TableBody>
          </Table>
          <FailedJobsTable
            items={failedItems}
            selectedRowId={selectedRowId}
            onSelect={(job) => {
              void selectRowForPreview(job)
            }}
            onShowFile={(job) => {
              void showFile(job.file_path)
            }}
          />
        </div>
      </div>

      <Dialog
        open={customDialogJobId !== null}
        onOpenChange={(open) => {
          if (!open) setCustomDialogJobId(null)
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Custom Schedule</DialogTitle>
            <DialogDescription>
              {customDialogJob
                ? displayCatalogPath(customDialogJob.file_path, rootPaths)
                : "Select custom date and time."}
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-3">
            <div className="grid gap-1">
              <Label htmlFor="decision-custom-date">Date</Label>
              <Input
                id="decision-custom-date"
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
              <Label htmlFor="decision-custom-time">Time</Label>
              <Input
                id="decision-custom-time"
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
                void scheduleCustom(customDialogJobId)
              }}
            >
              Save time
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
    </>
  )
}

function selectedPlatformsFromSelection(
  selection: PlatformSelection
): Array<"linkedin" | "x"> {
  const out: Array<"linkedin" | "x"> = []
  if (selection.linkedin) out.push("linkedin")
  if (selection.x) out.push("x")
  return out
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

function readyScheduleLabel(job: JobItem): string {
  if (!job.run_at_utc) return "Set schedule time"
  return formatRunAtLocal(job.run_at_utc)
}

function canScheduleFromSuggestion(
  job: JobItem,
  selection: PlatformSelection
): boolean {
  if (!job.run_at_utc) return false
  return selection.linkedin || selection.x
}

function decisionStatusRank(status: string): number {
  switch (status) {
    case "ready":
      return 0
    case "blocked":
      return 1
    case "canceled":
      return 2
    case "disabled":
      return 3
    default:
      return 99
  }
}

function selectedRowClass(selected: boolean): string {
  if (!selected) return "cursor-pointer hover:bg-muted/40"

  return [
    "cursor-pointer bg-blue-100/70 ring-1 ring-inset ring-blue-300",
    "hover:bg-blue-100/70 dark:bg-blue-950/40 dark:ring-blue-700",
  ].join(" ")
}

function formatRunAtLocal(rawUtc: string): string {
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
  return `${datePart}, ${timePart}`
  // const weekdayPart = new Intl.DateTimeFormat(undefined, {
  //   weekday: "short",
  // }).format(date)
  // return `${datePart}, ${timePart} (${weekdayPart})`
}

function isPastRunAt(rawUtc: string | null): boolean {
  if (!rawUtc) return false
  const time = Date.parse(rawUtc)
  if (Number.isNaN(time)) return false
  return time < Date.now()
}

async function checkReadyFileHasIssue(filePath: string): Promise<boolean> {
  try {
    const response = await fetch(`/api/catalog/preview?path=${encodeURIComponent(filePath)}`)
    if (!response.ok) return true
    const raw = await response.text()
    if (raw.trim().length === 0) return true
    const data: any = JSON.parse(raw)
    if (!data?.ok) return true
    return !data?.preview?.publishable
  } catch {
    return true
  }
}
