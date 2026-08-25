import { AlertTriangleIcon, CalendarClockIcon, ChevronDownIcon, SparklesIcon } from "lucide-react"

import { type SchedulePreset } from "@/lib/schedulePresets"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"

type ScheduleControlsProps = {
  presets: SchedulePreset[]
  disabled: boolean
  customLabel: string
  showAiIcon?: boolean
  showPastWarning?: boolean
  onPresetSelect: (preset: SchedulePreset) => void
  onCustomClick: () => void
}

export function ScheduleControls({
  presets,
  disabled,
  customLabel,
  showAiIcon = false,
  showPastWarning = false,
  onPresetSelect,
  onCustomClick,
}: ScheduleControlsProps) {
  return (
    <div className="flex items-center gap-2">
      <DropdownMenu>
        <DropdownMenuTrigger
          render={<Button size="icon-sm" variant="outline" disabled={disabled} />}
        >
          <ChevronDownIcon className="size-4" />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" sideOffset={6}>
          {presets.map((preset) => (
            <DropdownMenuItem
              key={preset.key}
              onClick={() => {
                onPresetSelect(preset)
              }}
            >
              {preset.label}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
      <Button size="sm" variant="outline" disabled={disabled} onClick={onCustomClick}>
        <span className="inline-flex items-center gap-2">
          <CalendarClockIcon className="size-4" />
          {customLabel}
          {showPastWarning ? (
            <AlertTriangleIcon className="size-4 text-amber-500" aria-label="schedule time is in the past" />
          ) : null}
          {showAiIcon ? <SparklesIcon className="size-4 text-emerald-500" /> : null}
        </span>
      </Button>
    </div>
  )
}
