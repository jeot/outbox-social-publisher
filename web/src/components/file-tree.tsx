import { ChevronRightIcon, FolderIcon } from "lucide-react"
import { useEffect, useRef, useState } from "react"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"
import {
  useCatalogStore,
  type CatalogJobBadge,
  type CatalogNode,
} from "@/store/catalogStore"

export function FileTree() {
  const roots = useCatalogStore((state) => state.roots)
  const loading = useCatalogStore((state) => state.loading)
  const error = useCatalogStore((state) => state.error)
  const loadCatalog = useCatalogStore((state) => state.loadCatalog)
  const badgesByPath = useCatalogStore((state) => state.badgesByPath)
  const selectedFilePath = useCatalogStore((state) => state.selectedFilePath)
  const selectFile = useCatalogStore((state) => state.selectFile)
  const expandedDirPaths = useCatalogStore((state) => state.expandedDirPaths)
  const highlightedFilePath = useCatalogStore((state) => state.highlightedFilePath)
  const highlightTick = useCatalogStore((state) => state.highlightTick)
  const setDirOpen = useCatalogStore((state) => state.setDirOpen)
  const clearTreeHighlight = useCatalogStore((state) => state.clearTreeHighlight)
  const fileElementRefs = useRef<Record<string, HTMLButtonElement | null>>({})
  const [openRootByPath, setOpenRootByPath] = useState<Record<string, boolean>>({})
  const readyFilePathSet = new Set(
    Object.entries(badgesByPath)
      .filter(([, badges]) => badges.some((badge) => badge.status === "ready"))
      .map(([path]) => path)
  )

  useEffect(() => {
    setOpenRootByPath((current) => {
      const next = { ...current }
      for (const entry of roots) {
        if (typeof next[entry.root] !== "boolean") next[entry.root] = true
      }
      return next
    })
  }, [roots])

  useEffect(() => {
    if (!highlightedFilePath) return
    const element = fileElementRefs.current[highlightedFilePath]
    if (element) {
      element.scrollIntoView({ block: "center", behavior: "smooth" })
    }
    setOpenRootByPath((current) => {
      const next = { ...current }
      for (const entry of roots) {
        if (highlightedFilePath === entry.root || highlightedFilePath.startsWith(`${entry.root}/`)) {
          next[entry.root] = true
        }
      }
      return next
    })
    const timer = window.setTimeout(() => {
      clearTreeHighlight()
    }, 3000)
    return () => window.clearTimeout(timer)
  }, [highlightTick, highlightedFilePath, clearTreeHighlight, roots])

  return (
    <div className="flex h-full flex-col border-l bg-card">
      <div className="flex items-center justify-between border-b px-3 py-2">
        <h3 className="text-sm font-semibold">Catalog</h3>
        <button
          type="button"
          onClick={() => void loadCatalog()}
          className="text-xs text-muted-foreground hover:text-foreground"
        >
          Refresh
        </button>
      </div>
      <div className="flex-1 overflow-auto p-2">
        {loading ? (
          <p className="px-2 text-xs text-muted-foreground">Loading…</p>
        ) : error ? (
          <p className="px-2 text-xs text-destructive">{error}</p>
        ) : roots.length === 0 ? (
          <p className="px-2 text-xs text-muted-foreground">
            No catalog roots configured.
          </p>
        ) : (
          <ul className="space-y-1">
            {roots.map((entry) => (
              <li key={entry.root} className="pt-1">
                <Collapsible
                  open={openRootByPath[entry.root] ?? true}
                  onOpenChange={(open) =>
                    setOpenRootByPath((current) => ({ ...current, [entry.root]: open }))
                  }
                  className="group/collapsible"
                >
                  <CollapsibleTrigger className="flex w-full items-center justify-between rounded-md bg-muted px-2 py-1.5 text-left text-[11px] font-bold tracking-wide text-muted-foreground uppercase hover:bg-muted">
                    <span className="min-w-0 truncate">{rootLabel(entry.root)}</span>
                    <span className="ml-2 flex items-center gap-2">
                      {entry.ok ? (
                        (() => {
                          const count = readyFileCountInNodeList(entry.tree, readyFilePathSet)
                          if (count < 1) return null
                          return (
                            <span className="min-w-5 rounded-full bg-emerald-500/15 px-1.5 py-0.5 text-center text-[10px] font-semibold text-emerald-700 normal-case">
                              {count}
                            </span>
                          )
                        })()
                      ) : null}
                      <ChevronRightIcon className="size-3 shrink-0 transition-transform duration-200 group-data-open/collapsible:rotate-90" />
                    </span>
                  </CollapsibleTrigger>
                  <CollapsibleContent className="pt-1">
                    {!entry.ok ? (
                      <p className="px-2 py-1 text-xs text-destructive">
                        {entry.error ?? "failed"}
                      </p>
                    ) : (
                      <ul className="space-y-1">
                        {entry.tree.map((node) => (
                          <TreeNode
                            key={node.path}
                            node={node}
                            badgesByPath={badgesByPath}
                            readyFilePathSet={readyFilePathSet}
                            selectedFilePath={selectedFilePath}
                            onSelectFile={selectFile}
                            expandedDirPaths={expandedDirPaths}
                            onSetDirOpen={setDirOpen}
                            highlightedFilePath={highlightedFilePath}
                            fileElementRefs={fileElementRefs.current}
                          />
                        ))}
                      </ul>
                    )}
                  </CollapsibleContent>
                </Collapsible>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  )
}

function rootLabel(fullPath: string): string {
  const normalized = fullPath.replace(/\/+$/, "")
  const parts = normalized.split(/[\\/]/).filter(Boolean)
  return parts[parts.length - 1] ?? fullPath
}

function TreeNode({
  node,
  badgesByPath,
  readyFilePathSet,
  selectedFilePath,
  onSelectFile,
  expandedDirPaths,
  onSetDirOpen,
  highlightedFilePath,
  fileElementRefs,
}: {
  node: CatalogNode
  badgesByPath: Record<string, CatalogJobBadge[]>
  readyFilePathSet: Set<string>
  selectedFilePath: string | null
  onSelectFile: (path: string) => Promise<void>
  expandedDirPaths: string[]
  onSetDirOpen: (path: string, open: boolean) => void
  highlightedFilePath: string | null
  fileElementRefs: Record<string, HTMLButtonElement | null>
}) {
  if (node.kind === "file") {
    const selected = selectedFilePath === node.path
    const highlighted = highlightedFilePath === node.path
    const badges = badgesByPath[node.path] ?? []
    const displayBadges = formatBadgesForDisplay(badges)
    return (
      <li>
        <button
          type="button"
          ref={(element) => {
            fileElementRefs[node.path] = element
          }}
          onClick={() => void onSelectFile(node.path)}
          className={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm ${
            highlighted
              ? "file-reveal-flash bg-yellow-100 ring-1 ring-yellow-400 dark:bg-yellow-900/30"
              : selected
                ? "bg-accent font-medium"
                : "hover:bg-accent"
          }`}
        >
          <span className="min-w-0 truncate">{node.name}</span>
          {displayBadges.length > 0 ? (
            <span className="ml-auto flex shrink-0 flex-wrap items-center justify-end gap-1">
              {displayBadges.map((badge, idx) => (
                <span
                  key={`${badge.label}-${idx}`}
                  className={`rounded-full px-2 py-0.5 text-[10px] font-semibold ${statusBadgeClassName(badge.status)}`}
                >
                  {badge.label}
                </span>
              ))}
            </span>
          ) : null}
        </button>
      </li>
    )
  }

  const readyCount = readyFileCountInNode(node, readyFilePathSet)
  const open = expandedDirPaths.includes(node.path)

  return (
    <li>
      <Collapsible open={open} onOpenChange={(next) => onSetDirOpen(node.path, next)}>
        <CollapsibleTrigger className="flex w-full items-center justify-between rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent">
          <span className="flex min-w-0 items-center gap-2">
            <FolderIcon className="size-4 shrink-0" />
            <span className="min-w-0 truncate">{node.name}</span>
          </span>
          <span className="ml-2 flex items-center gap-2">
            {readyCount > 0 ? (
              <span className="min-w-5 rounded-full bg-emerald-500/15 px-1.5 py-0.5 text-center text-[10px] font-semibold text-emerald-700">
                {readyCount}
              </span>
            ) : null}
            <ChevronRightIcon
              className={`size-4 shrink-0 transition-transform duration-200 ${open ? "rotate-90" : ""}`}
            />
          </span>
        </CollapsibleTrigger>
        <CollapsibleContent className="pl-8">
          <ul className="space-y-1">
            {(node.children ?? []).map((child) => (
            <TreeNode
              key={child.path}
              node={child}
              badgesByPath={badgesByPath}
              readyFilePathSet={readyFilePathSet}
              selectedFilePath={selectedFilePath}
              onSelectFile={onSelectFile}
              expandedDirPaths={expandedDirPaths}
              onSetDirOpen={onSetDirOpen}
              highlightedFilePath={highlightedFilePath}
              fileElementRefs={fileElementRefs}
              />
            ))}
          </ul>
        </CollapsibleContent>
      </Collapsible>
    </li>
  )
}

function readyFileCountInNode(node: CatalogNode, readyFilePathSet: Set<string>): number {
  if (node.kind === "file") {
    return readyFilePathSet.has(node.path) ? 1 : 0
  }
  return readyFileCountInNodeList(node.children ?? [], readyFilePathSet)
}

function readyFileCountInNodeList(
  nodes: CatalogNode[],
  readyFilePathSet: Set<string>
): number {
  let count = 0
  for (const node of nodes) {
    count += readyFileCountInNode(node, readyFilePathSet)
  }
  return count
}

function formatBadgesForDisplay(
  badges: CatalogJobBadge[]
): Array<{ status: string; label: string }> {
  if (badges.length === 0) return []
  const statuses = new Set(badges.map((badge) => badge.status))
  if (statuses.size === 1) {
    const status = badges[0].status
    const platforms = badges
      .map((badge) => displayPlatform(badge.platform))
      .filter((value, idx, arr) => value.length > 0 && arr.indexOf(value) === idx)
    if (platforms.length > 0) {
      return [{ status, label: `${statusLabel(status)}: ${platforms.join(", ")}` }]
    }
    return [{ status, label: statusLabel(status) }]
  }

  return badges.map((badge) => {
    const platform = displayPlatform(badge.platform)
    return {
      status: badge.status,
      label: platform.length > 0 ? `${platform} ${statusLabel(badge.status)}` : statusLabel(badge.status),
    }
  })
}

function displayPlatform(platform: string | null): string {
  if (!platform) return ""
  const lower = platform.trim().toLowerCase()
  if (lower === "x") return "X"
  if (lower === "linkedin") return "LinkedIn"
  return platform
}

function statusLabel(status: string): string {
  if (status.length < 1) return status
  return status.charAt(0).toUpperCase() + status.slice(1)
}

function statusBadgeClassName(status: string): string {
  switch (status) {
    case "ready":
      return "bg-emerald-600 text-white"
    case "scheduled":
      return "bg-blue-600 text-white"
    case "publishing":
      return "bg-indigo-600 text-white"
    case "published":
      return "bg-teal-600 text-white"
    case "blocked":
      return "bg-amber-600 text-white"
    case "failed":
      return "bg-red-600 text-white"
    case "canceled":
      return "bg-slate-600 text-white"
    case "disabled":
      return "bg-zinc-600 text-white"
    default:
      return "bg-muted text-foreground"
  }
}
