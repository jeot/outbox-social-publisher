import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { type PublishAttempt } from "@/store/catalogStore"

type PublishAttemptTimelineProps = {
  attempts: PublishAttempt[]
}

export function PublishAttemptTimeline({ attempts }: PublishAttemptTimelineProps) {
  if (attempts.length === 0) return null

  return (
    <Card className="mb-4" size="sm">
      <CardHeader>
        <CardTitle>Publish attempts</CardTitle>
        <CardDescription>Provider activity in chronological order.</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {attempts.map((attempt) => (
          <div
            key={attempt.attempt_no}
            className="border-l-2 border-muted pl-3 text-xs"
          >
            <Badge
              variant={
                attempt.finished_at === null
                  ? "outline"
                  : attempt.success
                    ? "secondary"
                    : "destructive"
              }
            >
              Attempt {attempt.attempt_no}: {attemptStatus(attempt)}
            </Badge>
            <div className="mt-2 space-y-1 text-muted-foreground">
              <p>Started: {formatAttemptTime(attempt.started_at)}</p>
              <p>Finished: {formatAttemptTime(attempt.finished_at)}</p>
              {attempt.request_id ? <p>Request: {attempt.request_id}</p> : null}
            </div>
            {attempt.error_message ? (
              <p className="mt-2 text-destructive">{attempt.error_message}</p>
            ) : null}
            {attempt.post_url ? (
              <Button
                className="mt-2 px-0"
                size="sm"
                variant="link"
                render={<a href={attempt.post_url} rel="noreferrer" target="_blank" />}
              >
                Open published post
              </Button>
            ) : null}
          </div>
        ))}
      </CardContent>
    </Card>
  )
}

function formatAttemptTime(value: string | null): string {
  return value ? new Date(value).toLocaleString() : "Pending"
}

function attemptStatus(attempt: PublishAttempt): string {
  if (attempt.finished_at === null) return "publishing"
  return attempt.success ? "published" : "failed"
}
