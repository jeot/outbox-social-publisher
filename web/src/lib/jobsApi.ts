export type JobItem = {
  id: string
  status: string
  platform: string | null
  selected_platforms?: string[]
  file_path: string
  run_at_utc: string | null
  timezone: string | null
  status_reason: string | null
  operator: string | null
  updated_at: string
}

async function parseJsonResponse(response: Response, label: string): Promise<any> {
  const raw = await response.text()
  if (raw.trim().length === 0) {
    throw new Error(`${label} returned empty response (status ${response.status})`)
  }

  let data: any
  try {
    data = JSON.parse(raw)
  } catch {
    throw new Error(`${label} returned non-JSON response (status ${response.status})`)
  }

  if (!response.ok || !data?.ok) {
    const fallback = scheduleMultiFailureSummary(data)
    throw new Error(
      data?.message ?? fallback ?? `${label} failed (status ${response.status})`
    )
  }

  return data
}

function scheduleMultiFailureSummary(data: any): string | null {
  if (data?.mode !== "job_schedule_multi") return null
  const results = Array.isArray(data?.results) ? data.results : []
  const failed: string[] = []
  for (const item of results) {
    const ok = Boolean(item?.result?.ok)
    if (ok) continue
    const platform =
      typeof item?.platform === "string" && item.platform.length > 0
        ? item.platform
        : "unknown"
    const reason =
      typeof item?.result?.message === "string" && item.result.message.length > 0
        ? item.result.message
        : "unknown error"
    failed.push(`${platform}: ${reason}`)
  }
  if (failed.length === 0) return "One or more selected platforms failed to schedule."
  return `One or more selected platforms failed to schedule (${failed.join("; ")})`
}

export async function listReadyJobs(): Promise<JobItem[]> {
  const response = await fetch("/api/jobs/ready")
  const data = await parseJsonResponse(response, "ready jobs API")
  return Array.isArray(data.items) ? data.items : []
}

export async function listScheduledJobs(): Promise<JobItem[]> {
  const response = await fetch("/api/jobs/scheduled")
  const data = await parseJsonResponse(response, "scheduled jobs API")
  return Array.isArray(data.items) ? data.items : []
}

export async function listBlockedJobs(): Promise<JobItem[]> {
  const response = await fetch("/api/jobs/blocked")
  const data = await parseJsonResponse(response, "blocked jobs API")
  return Array.isArray(data.items) ? data.items : []
}

export async function listCanceledJobs(): Promise<JobItem[]> {
  const response = await fetch("/api/jobs/canceled")
  const data = await parseJsonResponse(response, "canceled jobs API")
  return Array.isArray(data.items) ? data.items : []
}

export async function listDisabledJobs(): Promise<JobItem[]> {
  const response = await fetch("/api/jobs/disabled")
  const data = await parseJsonResponse(response, "disabled jobs API")
  return Array.isArray(data.items) ? data.items : []
}

async function postJson(path: string, payload: Record<string, unknown>, label: string): Promise<any> {
  const response = await fetch(path, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  })
  return parseJsonResponse(response, label)
}

export async function unreadyJob(id: string): Promise<void> {
  await postJson("/api/jobs/unready", { id }, "unready API")
}

export async function setJobPlatform(id: string, platform: "linkedin" | "x"): Promise<void> {
  await postJson("/api/jobs/platform/set", { id, platform }, "set platform API")
}

export async function clearJobPlatform(id: string): Promise<void> {
  await postJson("/api/jobs/platform/clear", { id }, "clear platform API")
}

export async function scheduleJob(
  id: string,
  at: string,
  timezone: string,
  platform?: "linkedin" | "x"
): Promise<any> {
  const payload: Record<string, unknown> = { id, at, timezone }
  if (platform) payload.platform = platform
  return postJson("/api/jobs/schedule", payload, "schedule API")
}

export async function scheduleJobMulti(
  id: string,
  at: string,
  timezone: string,
  platforms: Array<"linkedin" | "x">
): Promise<any> {
  const payload: Record<string, unknown> = { id, at, timezone, platforms }
  return postJson("/api/jobs/schedule-multi", payload, "schedule multi API")
}

export async function setReadyJobTime(
  id: string,
  at: string,
  timezone: string
): Promise<void> {
  await postJson("/api/jobs/time", { id, at, timezone }, "job time API")
}

export async function setScheduledJobTime(
  id: string,
  at: string,
  timezone: string
): Promise<void> {
  await postJson("/api/jobs/scheduled/time", { id, at, timezone }, "scheduled time API")
}

export async function setReadyJobPlatforms(
  id: string,
  platforms: Array<"linkedin" | "x">
): Promise<void> {
  await postJson("/api/jobs/platforms", { id, platforms }, "job platforms API")
}

export async function unscheduleJob(id: string): Promise<void> {
  await postJson("/api/jobs/unschedule", { id }, "unschedule API")
}

export async function cancelJob(id: string): Promise<void> {
  await postJson("/api/jobs/cancel", { id }, "cancel API")
}
