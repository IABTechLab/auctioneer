import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { once } from 'node:events';
import { chmod, mkdtemp, readFile, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

async function bounded(promise) {
  let timer;
  try {
    return await Promise.race([promise, new Promise((_resolve, reject) => {
      timer = setTimeout(() => reject(new Error('shutdown regression timed out')), 15_000);
    })]);
  } finally {
    clearTimeout(timer);
  }
}

for (const denyRemoval of [false, true]) test(
  denyRemoval ? 'SIGTERM preserves exit semantics and a safe diagnostic when config cleanup fails'
    : 'SIGTERM during browser startup reaps Axum and removes temporary config',
  async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'auctioneer-browser-shutdown-'));
  const child = spawn(process.execPath, [path.join(root, 'scripts/check-usersync-browser.mjs')], {
    cwd: root,
    env: {
      ...process.env,
      TMPDIR: dir, TMP: dir, TEMP: dir,
      AUCTIONEER__BROWSER__PLAYWRIGHT_MODULE: path.join(root, 'scripts/tests/fixtures/pending-browser.cjs'),
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let stderr = '';
  child.stderr.on('data', data => { stderr += data.toString(); });
  const exited = once(child, 'exit');
  let nativePids = [];
  let nativeAlive = true;
  try {
    await bounded(new Promise((resolve, reject) => {
      let output = '';
      child.once('error', reject);
      child.stdout.on('data', data => {
        output += data.toString();
        if (output.includes('browser-launch-pending')) resolve();
        else if (output.length > 512) reject(new Error('unexpected fixture startup output'));
      });
      exited.then(() => reject(new Error('harness exited before browser startup')), reject);
    }));
    const fixtures = await readdir(dir);
    assert.equal(fixtures.length, 1);
    const fixture = path.join(dir, fixtures[0]);
    const store = JSON.parse(await readFile(path.join(fixture, '.edgezero/local-config-auctioneer_runtime.json'), 'utf8'));
    const policy = JSON.parse(store.auctioneer_runtime).data;
    assert.equal(policy.schema_version, 1);
    const origin = policy.host.user_sync.external_url;
    assert.equal(new URL(origin).hostname, '127.0.0.1');
    // At this marker only Axum is a child; the stub never starts Chromium.
    nativePids = execFileSync('pgrep', ['-P', String(child.pid)], { encoding: 'utf8', timeout: 1_000 })
      .trim().split(/\s+/).map(Number);
    assert.equal(nativePids.length, 1);
    assert.ok(nativePids.every(pid => Number.isSafeInteger(pid) && pid > 0));
    if (denyRemoval) await chmod(dir, 0o500);
    assert.equal(child.kill('SIGTERM'), true);
    const [code] = await bounded(exited);
    try {
      const response = await fetch(`${origin}/status`, { signal: AbortSignal.timeout(200) });
      await response.arrayBuffer();
      nativeAlive = response.ok;
    } catch {
      nativeAlive = false;
    }
    assert.equal(nativeAlive, false, 'native Axum child must be stopped');
    for (const pid of nativePids) assert.throws(() => process.kill(pid, 0), { code: 'ESRCH' });
    assert.equal(code, 143, 'catchable termination must preserve SIGTERM exit semantics');
    if (denyRemoval) {
      assert.equal(stderr.trim(), 'Browser check cleanup did not fully complete');
      assert.ok((await readdir(dir)).length > 0, 'permission fixture actually refused removal');
    } else {
      assert.deepEqual(await readdir(dir), [], 'temporary config must be removed');
    }
  } finally {
    if (child.exitCode === null && child.signalCode === null) {
      child.kill('SIGKILL');
      await bounded(exited);
    }
    for (const pid of nativePids) {
      try { process.kill(pid, 'SIGKILL'); } catch (error) {
        if (error.code !== 'ESRCH') throw error;
      }
    }
    await chmod(dir, 0o700);
    await rm(dir, { recursive: true, force: true });
  }
});
