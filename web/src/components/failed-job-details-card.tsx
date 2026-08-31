import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { StatusBadge } from "@/components/status-badge"
import { type FailedJobDetails } from "@/store/catalogStore"

type FailedJobDetailsCardProps = {
  job: FailedJobDetails
}

export function FailedJobDetailsCard({ job }: FailedJobDetailsCardProps) {
  const latestAttempt = job.attempts[job.attempts.length - 1]

  return (
    <Card className="mt-4" size="sm">
      <CardHeader>
        <CardTitle>Failed publish</CardTitle>
        <CardDescription>Review the failed attempt before rescheduling.</CardDescription>
      </CardHeader>
      <CardContent className="space-y-3 text-sm">
        <div className="flex items-center gap-2">
          <StatusBadge status="failed" />
          <span className="text-muted-foreground">
            {job.platform ?? "Unknown platform"} · Attempt {latestAttempt?.attempt_no ?? job.attemptCount}
          </span>
        </div>
        <div>
          <p className="font-medium">Failed reason</p>
          <p className="mt-1 whitespace-pre-wrap break-words text-destructive">
            {latestAttempt?.error_message ?? job.statusReason ?? "Provider publish failed."}
          </p>
        </div>
        {latestAttempt ? (
          <div className="space-y-1 text-xs text-muted-foreground">
            <p>Started: {formatAttemptTime(latestAttempt.started_at)}</p>
            <p>Finished: {formatAttemptTime(latestAttempt.finished_at)}</p>
            {latestAttempt.error_type ? <p>Error type: {latestAttempt.error_type}</p> : null}
            {latestAttempt.request_id ? <p>Request: {latestAttempt.request_id}</p> : null}
          </div>
        ) : null}
      </CardContent>
    </Card>
  )
}

function formatAttemptTime(value: string | null): string {
  if (!value) return "Unknown"
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString()
}
