import { useEffect, useRef } from "react"
import { toast } from "sonner"

import { listScheduledJobs } from "@/lib/jobsApi"
import { playReminderBell } from "@/lib/reminderSound"

const POLL_INTERVAL_MS = 60_000
const TOAST_DURATION_MS = 30_000

export function useScheduledWorkerReminder() {
  const notifiedJobIds = useRef(new Set<string>())
  const checking = useRef(false)
  const audioWarningShown = useRef(false)

  useEffect(() => {
    let canceled = false

    const checkScheduledJobs = async () => {
      if (checking.current) return
      checking.current = true

      try {
        const jobs = await listScheduledJobs()
        if (canceled) return

        const now = Date.now()
        const dueJobIds = new Set(
          jobs
            .filter((job) => {
              if (!job.run_at_utc) return false
              const runAt = Date.parse(job.run_at_utc)
              return !Number.isNaN(runAt) && runAt <= now
            })
            .map((job) => job.id)
        )

        for (const jobId of notifiedJobIds.current) {
          if (!dueJobIds.has(jobId)) {
            notifiedJobIds.current.delete(jobId)
          }
        }

        const newlyDueIds = [...dueJobIds].filter(
          (jobId) => !notifiedJobIds.current.has(jobId)
        )
        if (newlyDueIds.length === 0) return

        const played = await playReminderBell()
        if (canceled) return

        if (played) {
          for (const jobId of newlyDueIds) {
            notifiedJobIds.current.add(jobId)
          }
          audioWarningShown.current = false
        } else if (!audioWarningShown.current) {
          audioWarningShown.current = true
          toast.warning(
            "A scheduled job is due, but browser audio is locked. Click Test bell in the sidebar.",
            { duration: TOAST_DURATION_MS }
          )
        }

        toast.info(dueReminderMessage(newlyDueIds.length), {
          duration: TOAST_DURATION_MS,
        })
      } catch {
        // The normal page APIs report connection errors; the reminder stays silent.
      } finally {
        checking.current = false
      }
    }

    void checkScheduledJobs()
    const intervalId = window.setInterval(checkScheduledJobs, POLL_INTERVAL_MS)

    return () => {
      canceled = true
      window.clearInterval(intervalId)
    }
  }, [])
}

function dueReminderMessage(count: number): string {
  if (count === 1) {
    return "A scheduled job is due. Run the supervised worker when ready."
  }

  return `${count} scheduled jobs are due. Run one supervised worker command at a time.`
}
