import { Badge } from "@/components/ui/badge"
import { statusBadgeClassName } from "@/lib/statusBadge"
import { cn } from "@/lib/utils"

export function StatusBadge({
  status,
  label,
  className,
}: {
  status: string
  label?: string
  className?: string
}) {
  return (
    <Badge
      variant="secondary"
      className={cn("capitalize", statusBadgeClassName(status), className)}
    >
      {label ?? status}
    </Badge>
  )
}
