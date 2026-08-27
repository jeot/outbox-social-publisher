import { ChevronRightIcon, FolderIcon } from "lucide-react"
import { useEffect, useRef, useState } from "react"
import { Badge } from "@/components/ui/badge"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"
import { StatusBadge } from "@/components/status-badge"
import { statusBadgeClassName } from "@/lib/statusBadge"
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
  const scheduledFilePathSet = new Set(
    Object.entries(badgesByPath)
      .filter(([, badges]) => badges.some((badge) => badge.status === "scheduled"))
      .map(([path]) => path)
  )
  const publishedFilePathSet = new Set(
    Object.entries(badgesByPath)
      .filter(([, badges]) => badges.some((badge) => badge.status === "published"))
      .map(([path]) => path)
  )
  const failedFilePathSet = new Set(
    Object.entries(badgesByPath)
      .filter(([, badges]) => badges.some((badge) => badge.status === "failed"))
      .map(([path]) => path)
  )
  const staleFilePathSet = new Set(
    Object.entries(badgesByPath)
      .filter(([, badges]) =>
        badges.some(
          (badge) =>
            badge.status === "blocked" ||
            badge.status === "canceled" ||
            badge.status === "disabled"
        )
      )
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
        if (
          highlightedFilePath === entry.root ||
          highlightedFilePath.startsWith(`${entry.root}/`)
        ) {
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
    <div className="flex h-full flex-col bg-card">
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
                    setOpenRootByPath((current) => ({
                      ...current,
                      [entry.root]: open,
                    }))
                  }
                  className="group/collapsible"
                >
                  <CollapsibleTrigger className="flex w-full items-center justify-between rounded-md bg-muted px-2 py-1.5 text-left text-[11px] font-bold tracking-wide text-muted-foreground uppercase hover:bg-muted">
                    <span className="min-w-0 truncate">{rootLabel(entry.root)}</span>
                    <span className="ml-2 flex items-center gap-2">
                      {entry.ok ? (
                        (() => {
                          const readyCount = fileCountInNodeList(
                            entry.tree,
                            readyFilePathSet
                          )
                          const scheduledCount = fileCountInNodeList(
                            entry.tree,
                            scheduledFilePathSet
                          )
                          const publishedCount = fileCountInNodeList(
                            entry.tree,
                            publishedFilePathSet
                          )
                          const staleCount = fileCountInNodeList(
                            entry.tree,
                            staleFilePathSet
                          )
                          const failedCount = fileCountInNodeList(
                            entry.tree,
                            failedFilePathSet
                          )
                          if (
                            readyCount < 1 &&
                            scheduledCount < 1 &&
                            publishedCount < 1 &&
                            failedCount < 1 &&
                            staleCount < 1
                          ) {
                            return null
                          }
                          return (
                            <>
                              {readyCount > 0 ? (
                                <FolderCountBadge status="ready">
                                  {readyCount}
                                </FolderCountBadge>
                              ) : null}
                              {scheduledCount > 0 ? (
                                <FolderCountBadge status="scheduled">
                                  {scheduledCount}
                                </FolderCountBadge>
                              ) : null}
                              {publishedCount > 0 ? (
                                <FolderCountBadge status="published">
                                  {publishedCount}
                                </FolderCountBadge>
                              ) : null}
                              {failedCount > 0 ? (
                                <FolderCountBadge status="failed">
                                  {failedCount}
                                </FolderCountBadge>
                              ) : null}
                              {staleCount > 0 ? (
                                <FolderCountBadge status="blocked">
                                  {staleCount}
                                </FolderCountBadge>
                              ) : null}
                            </>
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
                            scheduledFilePathSet={scheduledFilePathSet}
                            publishedFilePathSet={publishedFilePathSet}
                            failedFilePathSet={failedFilePathSet}
                            staleFilePathSet={staleFilePathSet}
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
  scheduledFilePathSet,
  publishedFilePathSet,
  failedFilePathSet,
  staleFilePathSet,
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
  scheduledFilePathSet: Set<string>
  publishedFilePathSet: Set<string>
  failedFilePathSet: Set<string>
  staleFilePathSet: Set<string>
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
                ? "bg-blue-100 ring-1 ring-blue-300 font-medium text-blue-900 dark:bg-blue-950/40 dark:text-blue-100 dark:ring-blue-700"
                : "hover:bg-accent"
          }`}
        >
          <span className="min-w-0 truncate">{node.name}</span>
          {displayBadges.length > 0 ? (
            <span className="ml-auto flex shrink-0 flex-wrap items-center justify-end gap-1">
              {displayBadges.map((badge, idx) => (
                <StatusBadge
                  key={`${badge.label}-${idx}`}
                  status={badge.status}
                  label={badge.label}
                  className="h-auto rounded-full px-2 py-0.5 text-[10px] font-semibold"
                />
              ))}
            </span>
          ) : null}
        </button>
      </li>
    )
  }

  const readyCount = fileCountInNode(node, readyFilePathSet)
  const scheduledCount = fileCountInNode(node, scheduledFilePathSet)
  const publishedCount = fileCountInNode(node, publishedFilePathSet)
  const failedCount = fileCountInNode(node, failedFilePathSet)
  const staleCount = fileCountInNode(node, staleFilePathSet)
  const open = expandedDirPaths.includes(node.path)

  return (
    <li>
      <Collapsible
        open={open}
        onOpenChange={(next) => onSetDirOpen(node.path, next)}
      >
        <CollapsibleTrigger className="flex w-full items-center justify-between rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent">
          <span className="flex min-w-0 items-center gap-2">
            <FolderIcon className="size-4 shrink-0" />
            <span className="min-w-0 truncate">{node.name}</span>
          </span>
          <span className="ml-2 flex items-center gap-2">
            {readyCount > 0 ? (
              <FolderCountBadge status="ready">{readyCount}</FolderCountBadge>
            ) : null}
            {scheduledCount > 0 ? (
              <FolderCountBadge status="scheduled">
                {scheduledCount}
              </FolderCountBadge>
            ) : null}
            {publishedCount > 0 ? (
              <FolderCountBadge status="published">
                {publishedCount}
              </FolderCountBadge>
            ) : null}
            {failedCount > 0 ? (
              <FolderCountBadge status="failed">
                {failedCount}
              </FolderCountBadge>
            ) : null}
            {staleCount > 0 ? (
              <FolderCountBadge status="blocked">
                {staleCount}
              </FolderCountBadge>
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
                scheduledFilePathSet={scheduledFilePathSet}
                publishedFilePathSet={publishedFilePathSet}
                failedFilePathSet={failedFilePathSet}
                staleFilePathSet={staleFilePathSet}
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

function FolderCountBadge({
  status,
  children,
}: {
  status: string
  children: number
}) {
  return (
    <Badge
      className={`min-w-5 justify-center rounded-full px-1.5 py-0.5 text-[10px] font-semibold normal-case ${statusBadgeClassName(status)}`}
    >
      {children}
    </Badge>
  )
}

function fileCountInNode(node: CatalogNode, filePathSet: Set<string>): number {
  if (node.kind === "file") {
    return filePathSet.has(node.path) ? 1 : 0
  }
  return fileCountInNodeList(node.children ?? [], filePathSet)
}

function fileCountInNodeList(
  nodes: CatalogNode[],
  filePathSet: Set<string>
): number {
  let count = 0
  for (const node of nodes) {
    count += fileCountInNode(node, filePathSet)
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
