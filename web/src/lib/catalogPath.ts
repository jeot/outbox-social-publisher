export function displayCatalogPath(
  absolutePath: string,
  catalogRoots: string[]
): string {
  if (!absolutePath || catalogRoots.length === 0) return absolutePath

  const matchedRoot = longestMatchingRoot(absolutePath, catalogRoots)
  if (!matchedRoot) return absolutePath

  const normalizedRoot = trimTrailingSlash(matchedRoot)
  const rootName = rootBaseName(normalizedRoot)
  if (absolutePath === normalizedRoot) return rootName

  const prefix = `${normalizedRoot}/`
  if (!absolutePath.startsWith(prefix)) return absolutePath
  const relative = absolutePath.slice(prefix.length)
  return relative.length > 0 ? `${rootName}/${relative}` : rootName
}

function longestMatchingRoot(path: string, roots: string[]): string | null {
  let best: string | null = null
  for (const root of roots) {
    const normalizedRoot = trimTrailingSlash(root)
    if (path === normalizedRoot || path.startsWith(`${normalizedRoot}/`)) {
      if (!best || normalizedRoot.length > best.length) {
        best = normalizedRoot
      }
    }
  }
  return best
}

function trimTrailingSlash(value: string): string {
  return value.replace(/\/+$/, "")
}

function rootBaseName(value: string): string {
  const parts = value.split("/").filter((part) => part.length > 0)
  return parts[parts.length - 1] ?? value
}
