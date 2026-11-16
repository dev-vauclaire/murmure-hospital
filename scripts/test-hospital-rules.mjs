import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import process from 'node:process';
import { spawnSync } from 'node:child_process';
import { fileURLToPath, URL } from 'node:url';
import { build } from 'vite';

const root = fileURLToPath(new URL('../', import.meta.url));
const temporaryDirectory = await mkdtemp(join(tmpdir(), 'murmure-hospital-rules-'));
const bundle = join(temporaryDirectory, 'hospital-formatting-rules.test.mjs');

try {
    // Bundle the existing TypeScript helpers and JSON fixtures for Node's test
    // runner, without loading app plugins, environment files or browser code.
    await build({
        root,
        configFile: false,
        envDir: temporaryDirectory,
        cacheDir: join(temporaryDirectory, 'cache'),
        publicDir: false,
        logLevel: 'error',
        build: {
            target: 'node22',
            outDir: temporaryDirectory,
            emptyOutDir: false,
            minify: false,
            lib: {
                entry: join(root, 'tests/hospital-formatting-rules.test.mjs'),
                formats: ['es'],
                fileName: () => 'hospital-formatting-rules.test.mjs',
            },
            rollupOptions: {
                external: (id) => id.startsWith('node:'),
            },
        },
    });

    // node:test starts its harness when the single bundled module executes.
    const result = spawnSync(process.execPath, [bundle], { stdio: 'inherit' });
    if (result.error) throw result.error;
    process.exitCode = result.status ?? 1;
} finally {
    await rm(temporaryDirectory, { recursive: true, force: true });
}
