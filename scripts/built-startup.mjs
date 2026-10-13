import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import ts from "typescript";

function localJsPath(specifier, importer, root, entry = false) {
  if (/^(?:[a-z]+:|\/\/)/i.test(specifier)) return null;
  const path = specifier.split(/[?#]/)[0];
  if (!path.endsWith(".js")) return null;
  if (path.startsWith("/")) return resolve(root, `.${path}`);
  if (entry || path.startsWith(".")) return resolve(dirname(importer), path);
  return null;
}

export function traceBuiltStartup(htmlPath) {
  const htmlFile = resolve(htmlPath);
  const root = dirname(htmlFile);
  const html = readFileSync(htmlFile, "utf8");
  const queue = [];
  for (const tag of html.matchAll(/<(?:script|link)\b[^>]*>/gi)) {
    if (!/\b(?:type=["']module["']|rel=["']modulepreload["'])/i.test(tag[0]))
      continue;
    const url = tag[0].match(/\b(?:src|href)=["']([^"']+)["']/i)?.[1];
    if (!url) continue;
    const file = localJsPath(url, htmlFile, root, true);
    if (file) queue.push(file);
  }
  if (queue.length === 0)
    throw new Error(`No startup JavaScript found in ${htmlFile}`);

  const seen = new Set();
  for (let i = 0; i < queue.length; i++) {
    const file = queue[i];
    if (seen.has(file)) continue;
    seen.add(file);
    const code = readFileSync(file, "utf8");
    const ast = ts.createSourceFile(
      file,
      code,
      ts.ScriptTarget.Latest,
      true,
      ts.ScriptKind.JS,
    );
    for (const statement of ast.statements) {
      if (
        !ts.isImportDeclaration(statement) &&
        !ts.isExportDeclaration(statement)
      )
        continue;
      const specifier = statement.moduleSpecifier;
      if (!specifier || !ts.isStringLiteral(specifier)) continue;
      const dependency = localJsPath(specifier.text, file, root);
      if (dependency) queue.push(dependency);
    }
  }
  return [...seen].sort();
}
