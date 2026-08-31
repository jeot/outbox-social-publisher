import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { ShowFileButton } from "@/components/show-file-button"
import { PlatformIcon } from "@/components/platform-icon"
import { StatusBadge } from "@/components/status-badge"
import { ScheduleControls } from "@/components/schedule-controls"
import { SCHEDULE_PRESETS, type SchedulePreset } from "@/lib/schedulePresets"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { fileNameFromPath } from "@/lib/catalogPath"
import { type JobItem } from "@/lib/jobsApi"

type FailedJobsTableProps = {
  items: JobItem[]
  selectedRowId: string | null
  onSelect: (job: JobItem) => void
  onShowFile: (job: JobItem) => void
  onSchedulePreset: (job: JobItem, preset: SchedulePreset) => void
  onCustomSchedule: (job: JobItem) => void
  onRemove: (job: JobItem) => void
  busyJobId: string | null
}

export function FailedJobsTable({
  items,
  selectedRowId,
  onSelect,
  onShowFile,
  onSchedulePreset,
  onCustomSchedule,
  onRemove,
  busyJobId,
}: FailedJobsTableProps) {
  if (items.length === 0) return null

  return (
    <section className="mt-8 border-t pt-6">
      <h3 className="text-base font-semibold text-destructive">Failed</h3>
      <p className="mt-1 text-sm text-muted-foreground">
        Provider requests that did not complete. Review the reason before scheduling
        again.
      </p>
      <Table className="mt-3 table-fixed">
        <TableHeader>
          <TableRow>
            <TableHead className="w-[15rem] min-w-[15rem]">File</TableHead>
            <TableHead>Platform</TableHead>
            <TableHead>Schedule</TableHead>
            <TableHead>Actions</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {items.map((job) => (
            <TableRow
              key={job.id}
              className={selectedRowClass(selectedRowId === job.id)}
              onClick={() => onSelect(job)}
            >
              <TableCell className="w-[15rem] min-w-[15rem] align-top">
                <div className="flex flex-col gap-1">
                  <span className="break-words">
                    {fileNameFromPath(job.file_path)}
                  </span>
                  <div className="flex flex-wrap gap-2">
                    <StatusBadge status="failed" />
                    <Badge variant="outline">{job.id.slice(0, 8)}</Badge>
                  </div>
                </div>
              </TableCell>
              <TableCell>
                <PlatformIcon platform={job.platform} />
              </TableCell>
              <TableCell>
                <ScheduleControls
                  presets={SCHEDULE_PRESETS}
                  disabled={busyJobId === job.id}
                  customLabel="Custom"
                  onPresetSelect={(preset) => onSchedulePreset(job, preset)}
                  onCustomClick={() => onCustomSchedule(job)}
                />
              </TableCell>
              <TableCell>
                <div
                  className="flex flex-wrap items-center gap-2"
                  onClick={(event) => event.stopPropagation()}
                >
                  <ShowFileButton onShowFile={() => onShowFile(job)} />
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={busyJobId === job.id}
                    onClick={() => onRemove(job)}
                  >
                    Remove
                  </Button>
                </div>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </section>
  )
}

function selectedRowClass(selected: boolean): string {
  return selected
    ? "cursor-pointer bg-blue-100/70 ring-1 ring-inset ring-blue-300 dark:bg-blue-950/40 dark:ring-blue-700"
    : "cursor-pointer hover:bg-muted/40"
}
