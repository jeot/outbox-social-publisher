import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"
import { ChevronRightIcon, FolderIcon } from "lucide-react"
import { useState } from "react"
import { useCatalogStore, type CatalogNode } from "@/store/catalogStore"

export function CatalogTreePanel({ className }: { className?: string }) {
  const roots = useCatalogStore((state) => state.roots)
  const loading = useCatalogStore((state) => state.loading)
  const error = useCatalogStore((state) => state.error)
  const loadCatalog = useCatalogStore((state) => state.loadCatalog)
  const selectedFilePath = useCatalogStore((state) => state.selectedFilePath)
  const selectFile = useCatalogStore((state) => state.selectFile)

  return (
    <aside className={className}>
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
                    defaultOpen
                    className="group/collapsible"
                  >
                    <CollapsibleTrigger className="flex w-full items-center justify-between rounded-md bg-muted px-2 py-1.5 text-left text-[11px] font-bold tracking-wide text-muted-foreground uppercase hover:bg-muted">
                      <span className="min-w-0 truncate">{rootLabel(entry.root)}</span>
                      <ChevronRightIcon className="size-3 shrink-0 transition-transform duration-200 group-data-open/collapsible:rotate-90" />
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
    </aside>
  )
}

function rootLabel(fullPath: string): string {
  const normalized = fullPath.replace(/\/+$/, "")
  const parts = normalized.split(/[\\/]/).filter(Boolean)
  return parts[parts.length - 1] ?? fullPath
}

function TreeNode({
  node,
  selectedFilePath,
  onSelectFile,
}: {
  node: CatalogNode
  selectedFilePath: string | null
  onSelectFile: (path: string) => Promise<void>
}) {
  const [open, setOpen] = useState(false)

  if (node.kind === "file") {
    const selected = selectedFilePath === node.path
    return (
      <li>
        <button
          type="button"
          onClick={() => void onSelectFile(node.path)}
          className={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm ${selected ? "bg-accent font-medium" : "hover:bg-accent"}`}
        >
          {/*<FileIcon className="size-4" />*/}
          <span className="min-w-0 truncate">{node.name}</span>
        </button>
      </li>
    )
  }

  return (
    <li>
      <Collapsible open={open} onOpenChange={setOpen}>
        <CollapsibleTrigger className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent">
          <ChevronRightIcon
            className={`size-4 shrink-0 transition-transform duration-200 ${open ? "rotate-90" : ""}`}
          />
          <FolderIcon className="size-4 shrink-0" />
          <span className="min-w-0 truncate">{node.name}</span>
        </CollapsibleTrigger>
        <CollapsibleContent className="pl-6">
          <ul className="space-y-1">
            {(node.children ?? []).map((child) => (
              <TreeNode
                key={child.path}
                node={child}
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
