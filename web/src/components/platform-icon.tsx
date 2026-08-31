type PlatformIconProps = {
  platform: string | null
}

export function PlatformIcon({ platform }: PlatformIconProps) {
  const normalized = platform?.trim().toLowerCase()
  const icon =
    normalized === "linkedin"
      ? "/linkedin-icon.webp"
      : normalized === "x"
        ? "/x-icon.webp"
        : normalized === "substack"
          ? "/substack-icon.png"
          : null

  if (!icon) {
    return (
      <span className="text-sm text-muted-foreground">
        {platform ?? "none"}
      </span>
    )
  }

  return (
    <img
      src={icon}
      alt={platform ?? normalized}
      title={platform ?? normalized}
      className="size-6 object-contain"
    />
  )
}
