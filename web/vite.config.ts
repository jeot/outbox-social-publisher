import { defineConfig } from 'vite'
import type { Plugin } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { fileURLToPath, URL } from 'node:url'
import fs from 'node:fs/promises'
import path from 'node:path'

type CatalogNode = {
  name: string
  path: string
  kind: 'file' | 'dir'
  children?: CatalogNode[]
}

function parseCatalogRoots(rawToml: string): string[] {
  const lines = rawToml.split(/\r?\n/)
  let inCatalog = false
  let collectingRoots = false
  let rootsBuffer = ""

  for (const line of lines) {
    const trimmed = line.trim()
    if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
      inCatalog = trimmed === "[catalog]"
      collectingRoots = false
      rootsBuffer = ""
      continue
    }
    if (!inCatalog) continue

    if (!collectingRoots) {
      const oneLine = trimmed.match(/^roots\s*=\s*\[(.*)\]\s*$/)
      if (oneLine) {
        rootsBuffer = oneLine[1]
        break
      }
      const start = trimmed.match(/^roots\s*=\s*\[(.*)$/)
      if (start) {
        collectingRoots = true
        rootsBuffer += start[1]
        if (start[1].includes("]")) {
          rootsBuffer = rootsBuffer.slice(0, rootsBuffer.indexOf("]"))
          collectingRoots = false
          break
        }
      }
      continue
    }

    rootsBuffer += trimmed
    if (trimmed.includes("]")) {
      rootsBuffer = rootsBuffer.slice(0, rootsBuffer.indexOf("]"))
      collectingRoots = false
      break
    }
  }

  if (!rootsBuffer) return []
  return rootsBuffer
    .split(',')
    .map((part) => part.trim())
    .filter((part) => part.startsWith('"') && part.endsWith('"'))
    .map((part) => part.slice(1, -1))
    .filter(Boolean)
}

async function buildTree(rootPath: string, depth = 0): Promise<CatalogNode[]> {
  if (depth > 4) return []
  const entries = await fs.readdir(rootPath, { withFileTypes: true })
  const sorted = entries.sort((a, b) => {
    if (a.isDirectory() && !b.isDirectory()) return -1
    if (!a.isDirectory() && b.isDirectory()) return 1
    return a.name.localeCompare(b.name)
  })

  const nodes: CatalogNode[] = []
  for (const entry of sorted) {
    if (entry.name.startsWith('.')) continue
    const full = path.join(rootPath, entry.name)
    if (entry.isDirectory()) {
      const children = await buildTree(full, depth + 1)
      nodes.push({
        name: entry.name,
        path: full,
        kind: 'dir',
        children,
      })
      continue
    }
    if (entry.isFile() && entry.name.toLowerCase().endsWith('.md')) {
      nodes.push({
        name: entry.name,
        path: full,
        kind: 'file',
      })
    }
  }
  return nodes
}

function catalogApiPlugin(): Plugin {
  return {
    name: 'publo-catalog-api',
    configureServer(server) {
      server.middlewares.use('/api/catalog/tree', async (_req, res) => {
        try {
          const workspaceRoot = fileURLToPath(new URL('.', import.meta.url))
          const repoRoot = path.resolve(workspaceRoot, '..')
          const configPath = path.join(repoRoot, 'config.toml')
          const configRaw = await fs.readFile(configPath, 'utf8')
          const roots = parseCatalogRoots(configRaw)

          const result = await Promise.all(
            roots.map(async (root) => {
              try {
                const tree = await buildTree(root)
                return { root, ok: true, tree }
              } catch (err) {
                return {
                  root,
                  ok: false,
                  tree: [] as CatalogNode[],
                  error: err instanceof Error ? err.message : 'unknown_error',
                }
              }
            })
          )

          res.setHeader('Content-Type', 'application/json')
          res.end(JSON.stringify({ ok: true, roots: result }))
        } catch (err) {
          res.statusCode = 500
          res.setHeader('Content-Type', 'application/json')
          res.end(
            JSON.stringify({
              ok: false,
              message: err instanceof Error ? err.message : 'failed_to_build_catalog',
            })
          )
        }
      })
    },
  }
}

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss(), catalogApiPlugin()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
})
