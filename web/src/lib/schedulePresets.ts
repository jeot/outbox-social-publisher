export type SchedulePreset = {
  key: string
  label: string
  at: () => Date
}

export const SCHEDULE_PRESETS: SchedulePreset[] = [
  { key: "today_09", label: "Today 09:00", at: () => localAt(0, 9) },
  { key: "today_12", label: "Today 12:00", at: () => localAt(0, 12) },
  { key: "today_16", label: "Today 16:00", at: () => localAt(0, 16) },
  { key: "today_19", label: "Today 19:00", at: () => localAt(0, 19) },
  { key: "tomorrow_09", label: "Tomorrow 09:00", at: () => localAt(1, 9) },
  { key: "tomorrow_12", label: "Tomorrow 12:00", at: () => localAt(1, 12) },
  { key: "tomorrow_16", label: "Tomorrow 16:00", at: () => localAt(1, 16) },
  { key: "tomorrow_19", label: "Tomorrow 19:00", at: () => localAt(1, 19) },
  { key: "next_week_09", label: "Next week 09:00", at: () => localAt(7, 9) },
  { key: "next_week_12", label: "Next week 12:00", at: () => localAt(7, 12) },
  { key: "next_week_16", label: "Next week 16:00", at: () => localAt(7, 16) },
  { key: "next_week_19", label: "Next week 19:00", at: () => localAt(7, 19) },
  { key: "plus_5m", label: "+5m", at: () => plusMinutes(5) },
  { key: "plus_30m", label: "+30m", at: () => plusMinutes(30) },
  { key: "plus_1h", label: "+1h", at: () => plusMinutes(60) },
  { key: "plus_3h", label: "+3h", at: () => plusMinutes(180) },
]

function localAt(dayOffset: number, hour: number): Date {
  const next = new Date()
  next.setDate(next.getDate() + dayOffset)
  next.setHours(hour, 0, 0, 0)
  return next
}

function plusMinutes(minutes: number): Date {
  const next = new Date()
  next.setMinutes(next.getMinutes() + minutes)
  return next
}
