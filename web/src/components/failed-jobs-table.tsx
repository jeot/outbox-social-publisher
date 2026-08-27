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
import { type JobItem } from "@/lib/jobsApi"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"

type FailedJobsTableProps = {
  items: JobItem[]
  selectedRowId: string | null
  onSelect: (job: JobItem) => void
  onShowFile: (job: JobItem) => void
}

export function FailedJobsTable({
  items,
  selectedRowId,
  onSelect,
  onShowFile,
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
            <TableHead>Reason</TableHead>
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
                    <Badge variant="destructive">failed</Badge>
                    <Badge variant="outline">{job.id.slice(0, 8)}</Badge>
                  </div>
                </div>
              </TableCell>
              <TableCell>
                <Badge variant="secondary">{job.platform ?? "none"}</Badge>
              </TableCell>
              <TableCell className="text-sm text-destructive">
                {job.status_reason ?? "Provider publish failed."}
              </TableCell>
              <TableCell>
                <div onClick={(event) => event.stopPropagation()}>
                  <Tooltip>
                    <TooltipTrigger
                      render={
                        <Button
                          size="icon-sm"
                          variant="outline"
                          onClick={() => onShowFile(job)}
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
