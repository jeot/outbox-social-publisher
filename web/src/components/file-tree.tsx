import { AstroidIcon, ChevronRightIcon, FolderIcon } from "lucide-react"
import { useState } from "react"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"
import { useCatalogStore, type CatalogNode } from "@/store/catalogStore"

export function FileTree() {
  const roots = useCatalogStore((state) => state.roots)
  const loading = useCatalogStore((state) => state.loading)
  const error = useCatalogStore((state) => state.error)
  const loadCatalog = useCatalogStore((state) => state.loadCatalog)
  const readyByPath = useCatalogStore((state) => state.readyByPath)
  const selectedFilePath = useCatalogStore((state) => state.selectedFilePath)
  const selectFile = useCatalogStore((state) => state.selectFile)
  const readyFilePathSet = new Set(Object.keys(readyByPath))

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
                <Collapsible defaultOpen className="group/collapsible">
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
                            readyByPath={readyByPath}
                            readyFilePathSet={readyFilePathSet}
                            selectedFilePath={selectedFilePath}
                            onSelectFile={selectFile}
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
  readyByPath,
  readyFilePathSet,
  selectedFilePath,
  onSelectFile,
}: {
  node: CatalogNode
  readyByPath: Record<string, string | null>
  readyFilePathSet: Set<string>
  selectedFilePath: string | null
  onSelectFile: (path: string) => Promise<void>
}) {
  const [open, setOpen] = useState(false)

  if (node.kind === "file") {
    const selected = selectedFilePath === node.path
    const isReady = readyFilePathSet.has(node.path)
    const isAiReady = isReady && readyByPath[node.path] === "ai"
    return (
      <li>
        <button
          type="button"
          onClick={() => void onSelectFile(node.path)}
          className={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm ${selected ? "bg-accent font-medium" : "hover:bg-accent"}`}
        >
          <span className="min-w-0 truncate">{node.name}</span>
          {isReady ? (
            <span className="ml-auto shrink-0 rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-semibold text-emerald-700 inline-flex items-center gap-1">
              Ready
              {isAiReady ? <AstroidIcon className="size-3" /> : null}
            </span>
          ) : null}
        </button>
      </li>
    )
  }

  const readyCount = readyFileCountInNode(node, readyFilePathSet)

  return (
    <li>
      <Collapsible open={open} onOpenChange={setOpen}>
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
              readyByPath={readyByPath}
              readyFilePathSet={readyFilePathSet}
              selectedFilePath={selectedFilePath}
              onSelectFile={onSelectFile}
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
