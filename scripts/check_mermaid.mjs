// Parse all authored Mermaid diagrams with the real Mermaid parser.
import { readFile, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL, fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const toolRoot = path.resolve(process.argv[2] ?? path.join(root, '.tmp/docs-tools'));
const requireTools = createRequire(path.join(toolRoot, 'package.json'));
const { JSDOM } = requireTools('jsdom');
const dom = new JSDOM('<!doctype html><html><body></body></html>');
globalThis.window = dom.window;
globalThis.document = dom.window.document;
const mermaidEntry = requireTools.resolve('mermaid');
const { default: mermaid } = await import(pathToFileURL(mermaidEntry).href);
mermaid.initialize({ startOnLoad: false, securityLevel: 'strict' });

async function* files(directory) {
  for (const item of await readdir(directory, { withFileTypes: true })) {
    const full = path.join(directory, item.name);
    if (item.isDirectory()) yield* files(full);
    else if (item.name.endsWith('.md')) yield full;
  }
}

let count = 0;
let failed = 0;
const diagrams = [];
for await (const file of files(path.join(root, 'docs'))) {
  const source = await readFile(file, 'utf8');
  let index = 0;
  for (const match of source.matchAll(/```mermaid\s*\n([\s\S]*?)```/g)) {
    index += 1;
    try {
      await mermaid.parse(match[1]);
      diagrams.push({ file: path.relative(root, file), index, source: match[1] });
      count += 1;
    } catch (error) {
      failed += 1;
      console.error(`${path.relative(root, file)} diagram ${index}: ${error.message}`);
    }
  }
}
if (count === 0) throw new Error('No diagrams found');
if (process.env.MERMAID_EXPORT) {
  await writeFile(process.env.MERMAID_EXPORT, JSON.stringify(diagrams, null, 2));
}
console.log(`Mermaid parsed: ${count}, failures: ${failed}`);
process.exitCode = failed ? 1 : 0;
