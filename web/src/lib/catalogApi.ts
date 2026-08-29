export async function openCatalogFile(
  path: string,
  app: "default" | "obsidian"
): Promise<void> {
  const response = await fetch("/api/catalog/open", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ path, app }),
  })
  const raw = await response.text()
  let data: any = null

  try {
    data = raw.length > 0 ? JSON.parse(raw) : null
  } catch {
    throw new Error(
      `open file API returned non-JSON response (status ${response.status})`
    )
  }

  if (!response.ok || !data?.ok) {
    throw new Error(
      data?.message ?? `open file API failed (status ${response.status})`
    )
  }
}
