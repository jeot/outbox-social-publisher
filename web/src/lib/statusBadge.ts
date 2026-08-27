export function statusBadgeClassName(status: string): string {
  switch (status) {
    case "ready":
      return "bg-emerald-500/15 text-emerald-700 dark:text-emerald-300"
    case "scheduled":
      return "bg-blue-500/15 text-blue-700 dark:text-blue-300"
    case "publishing":
      return "bg-indigo-600 text-white dark:bg-indigo-500 dark:text-indigo-950"
    case "published":
      return "bg-teal-600 text-white dark:bg-teal-500 dark:text-teal-950"
    case "failed":
      return "bg-red-600 text-white dark:bg-red-500 dark:text-red-950"
    case "blocked":
    case "canceled":
    case "disabled":
      return "bg-rose-500/15 text-rose-700 dark:text-rose-300"
    default:
      return "bg-muted text-foreground"
  }
}
