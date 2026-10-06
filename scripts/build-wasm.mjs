import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
const root = resolve(import.meta.dirname, '..');
const run = (command, args) => {
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit', shell: false });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} failed (${result.status})`);
};
run('cargo', ['build', '--locked', '--release', '-p', 'liar-wasm', '--lib', '--target', 'wasm32-unknown-unknown']);
const local = resolve(root, '.tmp/wasm-tools/bin', process.platform === 'win32' ? 'wasm-bindgen.exe' : 'wasm-bindgen');
const cli = process.env.LIAR_WASM_BINDGEN || (existsSync(local) ? local : 'wasm-bindgen');
run(cli, ['target/wasm32-unknown-unknown/release/liar_wasm.wasm', '--target', 'web', '--out-dir', 'apps/web/public/core']);
const source = readFileSync(resolve(root, 'apps/web/public/core/liar_wasm.d.ts'), 'utf8');
const target = resolve(root, 'packages/core-bridge/src/wasm-bindings.d.ts');
if (process.argv.includes('--check')) {
  if (!existsSync(target) || readFileSync(target, 'utf8') !== source) throw new Error('WASM declarations are stale; run pnpm wasm:generate');
} else {
  mkdirSync(resolve(target, '..'), { recursive: true });
  writeFileSync(target, source);
}
