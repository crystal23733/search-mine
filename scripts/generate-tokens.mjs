import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
const root = resolve(import.meta.dirname, '..');
const tokens = JSON.parse(readFileSync(resolve(root, 'packages/design-tokens/tokens.json'), 'utf8'));
for (const value of [...Object.values(tokens.colors), ...Object.values(tokens['high-contrast'])]) if (!/^#[0-9a-f]{6}$/.test(value)) throw new Error('Invalid color token');
const declarations = (section, prefix, unit = '') => Object.entries(section).map(([name, value]) => `  --${prefix}-${name}: ${value}${unit};`).join('\n');
const css = `/* Generated from tokens.json; do not edit. */\n:root {\n${declarations(tokens.colors, 'color')}\n${declarations(tokens.space, 'space', 'px')}\n${declarations(tokens.radius, 'radius', 'px')}\n}\n[data-contrast="high"] {\n${declarations(tokens['high-contrast'], 'color')}\n}\n`;
const colors = Object.fromEntries(Object.entries(tokens.colors).map(([key, value]) => [key, Number.parseInt(value.slice(1), 16)]));
const highColors = Object.fromEntries(Object.entries({ ...tokens.colors, ...tokens['high-contrast'] }).map(([key, value]) => [key, Number.parseInt(value.slice(1), 16)]));
const ts = `// Generated from tokens.json; do not edit.\nexport const COLORS = ${JSON.stringify(colors, null, 2)} as const;\nexport const HIGH_CONTRAST_COLORS = ${JSON.stringify(highColors, null, 2)} as const;\n`;
for (const [name, source] of [['packages/design-tokens/src/tokens.css', css], ['packages/design-tokens/src/index.ts', ts]]) {
 const target = resolve(root, name);
 if (process.argv.includes('--check')) { if (readFileSync(target, 'utf8') !== source) throw new Error(`Stale token output: ${name}`); }
 else { mkdirSync(resolve(target, '..'), { recursive: true }); writeFileSync(target, source); }
}
