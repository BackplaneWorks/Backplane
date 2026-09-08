type Source = { filename: string; content: string };

function resolveSheet(parent: string, child: string) {
  const parts = parent.split("/").slice(0, -1);
  for (const part of child.replaceAll("\\", "/").split("/")) {
    if (part === "..") parts.pop();
    else if (part && part !== ".") parts.push(part);
  }
  return parts.join("/");
}

/** Unrelated schematics can override Prism's root-sheet selection. */
export async function loadSchematicSources(
  root: string,
  available: readonly string[],
  read: (path: string) => Promise<string>,
): Promise<Source[]> {
  const sources: Source[] = [];
  const visited = new Set<string>();
  const files = new Set(available);
  const visit = async (filename: string) => {
    if (visited.has(filename)) return;
    visited.add(filename);
    const content = await read(filename);
    sources.push({ filename, content });
    for (const match of content.matchAll(/\(property\s+"Sheetfile"\s+"((?:\\.|[^"\\])*)"/g)) {
      const child = resolveSheet(filename, JSON.parse(`"${match[1]}"`) as string);
      if (!files.has(child)) throw new Error(`Referenced schematic sheet not found: ${child}`);
      await visit(child);
    }
  };
  await visit(root);
  return sources;
}
