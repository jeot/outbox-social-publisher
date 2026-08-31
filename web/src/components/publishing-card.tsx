import { Badge } from "@/components/ui/badge"
import { ShowFileButton } from "@/components/show-file-button"
import { PlatformIcon } from "@/components/platform-icon"
import { StatusBadge } from "@/components/status-badge"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
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

type PublishingCardProps = {
  items: JobItem[]
  onShowFile: (job: JobItem) => void
}

export function PublishingCard({
  items,
  onShowFile,
}: PublishingCardProps) {
  if (items.length === 0) return null

  return (
    <Card className="mb-6 border-blue-200 bg-blue-50/60 dark:border-blue-900 dark:bg-blue-950/20">
      <CardHeader>
        <CardTitle className="text-blue-900 dark:text-blue-100">
          Publishing
        </CardTitle>
        <CardDescription>
          These jobs are owned by a worker. Do not reschedule them.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="min-w-[15rem]">File</TableHead>
              <TableHead>Platform</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {items.map((job) => (
              <TableRow key={job.id}>
                <TableCell className="min-w-[15rem] align-top">
                  <div className="flex flex-col gap-1">
                    <span className="break-words">
                      {fileNameFromPath(job.file_path)}
                    </span>
                    <Badge className="w-fit" variant="outline">
                      {job.id.slice(0, 8)}
                    </Badge>
                  </div>
                </TableCell>
                <TableCell>
                  <PlatformIcon platform={job.platform} />
                </TableCell>
                <TableCell>
                  <StatusBadge status="publishing" />
                </TableCell>
                <TableCell>
                  <ShowFileButton onShowFile={() => onShowFile(job)} />
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  )
}
