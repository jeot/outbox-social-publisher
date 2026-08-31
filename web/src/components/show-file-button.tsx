import { Link2Icon } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"

type ShowFileButtonProps = {
  onShowFile: () => void
  disabled?: boolean
}

export function ShowFileButton({
  onShowFile,
  disabled = false,
}: ShowFileButtonProps) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            size="icon-sm"
            variant="outline"
            disabled={disabled}
            aria-label="Show the file"
            onClick={(event) => {
              event.stopPropagation()
              onShowFile()
            }}
          />
        }
      >
        <Link2Icon className="size-4" />
      </TooltipTrigger>
      <TooltipContent>show the file</TooltipContent>
    </Tooltip>
  )
}
