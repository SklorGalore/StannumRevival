import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(scriptDirectory, '..');
const tauriCli = resolve(projectRoot, 'node_modules/@tauri-apps/cli/tauri.js');

if (!existsSync(tauriCli)) {
  console.error(`Tauri CLI entry point not found: ${tauriCli}`);
  process.exit(1);
}

const env = { ...process.env };
if (process.platform === 'darwin' && env.CI === undefined) {
  env.CI = 'true';
}

const result = spawnSync(process.execPath, [tauriCli, 'build', ...process.argv.slice(2)], {
  cwd: projectRoot,
  env,
  stdio: 'inherit',
});

if (result.error) {
  console.error(`Failed to launch Tauri CLI: ${result.error.message}`);
  process.exit(1);
}

if (result.signal) {
  process.kill(process.pid, result.signal);
} else {
  process.exit(result.status ?? 1);
}
