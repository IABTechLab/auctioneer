// Actual-browser integration against the existing generated-App Axum binary.
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import http from 'node:http';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';

const require = createRequire(import.meta.url);
let chromium;
try {
  ({ chromium } = require(process.env.AUCTIONEER__BROWSER__PLAYWRIGHT_MODULE || 'playwright'));
} catch {
  throw new Error('Install Playwright and Chromium, or set AUCTIONEER__BROWSER__PLAYWRIGHT_MODULE to an installed Playwright module path.');
}
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const timeout = 10_000;
const metadata = JSON.parse(execFileSync('cargo', ['metadata', '--no-deps', '--format-version', '1', '--locked', '--offline'], { cwd: root, timeout, maxBuffer: 1024 * 1024 }));
const binary = path.join(metadata.target_directory, 'debug', 'auctioneer-adapter-axum');
const configCommand = path.join(metadata.target_directory, 'debug', 'examples', 'usersync_browser_config');
const syntheticUid = 'browser-fixture-user';
let callbackTarget;
const stop = new AbortController();
let interrupted;
const exitCodes = { SIGINT: 130, SIGTERM: 143, SIGHUP: 129 };
const signalHandlers = new Map(Object.keys(exitCodes).map(signal => [signal, () => {
  interrupted ||= signal;
  stop.abort();
}]));

function checkRunning() {
  if (stop.signal.aborted) throw new Error('Browser check interrupted');
}

async function untilStopped(action) {
  checkRunning();
  const promise = action();
  let abort;
  try {
    return await bounded(Promise.race([
      promise,
      new Promise((_resolve, reject) => {
        abort = () => reject(new Error('Browser check interrupted'));
        stop.signal.addEventListener('abort', abort, { once: true });
      }),
    ]), 'browser operation');
  } finally {
    stop.signal.removeEventListener('abort', abort);
  }
}

async function listen(server) {
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  checkRunning();
  return `http://127.0.0.1:${server.address().port}`;
}

async function closeServer(server) {
  if (!server.listening) return;
  server.closeAllConnections();
  await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
}

function publisher() {
  return http.createServer((request, response) => {
    if (request.url === '/callback' && callbackTarget) {
      response.writeHead(302, { location: callbackTarget, 'cache-control': 'no-store' });
      response.end();
      return;
    }
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store' });
    response.end('<!doctype html><title>Publisher sync fixture</title>');
  });
}

async function stopChild(child) {
  if (!child?.pid || child.exitCode !== null || child.signalCode !== null) return;
  const exited = once(child, 'exit');
  child.kill('SIGKILL');
  await bounded(exited, 'Axum child cleanup', 3_000);
}

async function bounded(promise, label, milliseconds = timeout) {
  let timer;
  try {
    return await Promise.race([
      promise,
      new Promise((_resolve, reject) => {
        timer = setTimeout(() => reject(new Error(`${label} timed out`)), milliseconds);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

function assertHeaders(headers, origin) {
  assert.equal(headers['cache-control'], 'no-store');
  assert.equal(headers['access-control-allow-origin'], origin);
  assert.equal(headers['access-control-allow-credentials'], 'true');
  // CDP joins duplicate header fields with newlines rather than commas.
  assert.ok(headers.vary.split(/[,\n]/).some(value => value.trim().toLowerCase() === 'origin'));
}

async function browserFetch(page, url, method = 'GET', body, json = true) {
  return untilStopped(() => page.evaluate(async ({ url, method, body, json }) => {
    try {
      const response = await fetch(url, {
        method,
        credentials: 'include',
        ...(body === undefined ? {} : {
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify(body),
        }),
        signal: AbortSignal.timeout(5_000),
      });
      return { status: response.status, body: await (json ? response.json() : response.text()) };
    } catch (error) {
      return { error: error.name };
    }
  }, { url, method, body, json }));
}

const allowed = publisher();
const denied = publisher();
// Non-loopback browser traffic terminates here; this proxy never forwards it.
const blockedProxy = http.createServer((_request, response) => {
  response.writeHead(502);
  response.end();
});
blockedProxy.on('connect', (_request, socket) => socket.destroy());
let child;
let browser;
let browserStartup;
let dir;
let childError;
let cleaning;
let evidence;

function cleanup() {
  cleaning ||= (async () => {
    const results = await Promise.allSettled([
      bounded(browserStartup?.then(created => created.close(), () => undefined), 'browser cleanup'),
      stopChild(child),
      bounded(closeServer(allowed), 'publisher cleanup'),
      bounded(closeServer(denied), 'denied publisher cleanup'),
      bounded(closeServer(blockedProxy), 'proxy cleanup'),
    ]);
    if (dir) await bounded(rm(dir, { recursive: true, force: true }), 'temporary storage cleanup');
    return results.find(result => result.status === 'rejected');
  })();
  return cleaning;
}

for (const [signal, handler] of signalHandlers) process.on(signal, handler);
try {
  const allowedOrigin = await listen(allowed);
  const deniedOrigin = await listen(denied);
  const proxyOrigin = await listen(blockedProxy);
  const reserve = http.createServer();
  const syncOrigin = await listen(reserve);
  await closeServer(reserve);
  const { envelope, fixture } = JSON.parse(execFileSync(configCommand, [allowedOrigin, syncOrigin], { encoding: 'utf8', timeout }));
  assert.equal(fixture.bidder, 'fake_browser');
  assert.equal(fixture.sync_type, 'redirect');
  assert.ok(fixture.family && fixture.family !== fixture.bidder);
  assert.ok(fixture.user_macro);
  const callbackQuery = new URLSearchParams({
    bidder: fixture.bidder, gdpr: '0', gdpr_consent: '', us_privacy: '',
    gpp: '', gpp_sid: '', f: 'i', account: 'publisher',
  });
  const expectedCallback = `${syncOrigin}/setuid?${callbackQuery}&uid=${fixture.user_macro}`;
  const encodedCallback = new URLSearchParams({ callback: expectedCallback }).toString().slice('callback='.length);
  assert.ok(fixture.endpoint_template.endsWith('?{{.RedirectURL}}'));
  const expectedSyncUrl = fixture.endpoint_template.replace('{{.RedirectURL}}', encodedCallback);
  dir = await mkdtemp(path.join(tmpdir(), 'auctioneer-usersync-browser-'));
  checkRunning();
  await mkdir(path.join(dir, '.edgezero'));
  await writeFile(path.join(dir, '.edgezero', 'local-config-auctioneer_runtime.json'), JSON.stringify({ auctioneer_runtime: JSON.stringify(envelope) }));
  checkRunning();
  child = spawn(binary, [], {
    cwd: dir,
    env: {
      EDGEZERO__ADAPTER__HOST: '127.0.0.1',
      EDGEZERO__ADAPTER__PORT: new URL(syncOrigin).port,
      // Public synthetic fixture, never an operator/deployment default.
      EDGEZERO__SECRETS__USERSYNC_COOKIE_KEYRING: JSON.stringify({
        active: 'test', keys: [{ id: 'test', key: Buffer.alloc(32, 1).toString('base64url') }],
      }),
    },
    stdio: 'ignore',
  });
  child.on('error', error => { childError = error; });
  const deadline = Date.now() + timeout;
  for (;;) {
    checkRunning();
    if (childError) throw childError;
    if (child.exitCode !== null || child.signalCode !== null) {
      throw new Error('Axum child exited during bootstrap; rebuild binary and example with --features insecure-bidders.');
    }
    try {
      const ready = await fetch(`${syncOrigin}/status`, { signal: AbortSignal.timeout(200) });
      await ready.arrayBuffer();
      if (ready.ok) break;
    } catch { /* Retry only inside the startup deadline. */ }
    assert.ok(Date.now() < deadline, 'Axum readiness timed out');
    await delay(25, undefined, { signal: stop.signal });
  }

  // This script owns signal cleanup, including a launch still in progress.
  checkRunning();
  browserStartup = chromium.launch({
    timeout, proxy: { server: proxyOrigin, bypass: '127.0.0.1' },
    handleSIGINT: false, handleSIGTERM: false, handleSIGHUP: false,
  });
  browser = await untilStopped(() => browserStartup);
  const context = await untilStopped(() => browser.newContext());
  const page = await untilStopped(() => context.newPage());
  page.setDefaultTimeout(timeout);
  page.setDefaultNavigationTimeout(timeout);
  // Observe native preflights without routing/interception, which can synthesize them.
  const protocol = await untilStopped(() => context.newCDPSession(page));
  const networkRequests = new Map();
  const preflights = [];
  const blockedCookies = [];
  protocol.on('Network.requestWillBeSent', event => networkRequests.set(event.requestId, event.request));
  protocol.on('Network.responseReceived', event => {
    if (networkRequests.get(event.requestId)?.method === 'OPTIONS') preflights.push(event.response);
  });
  protocol.on('Network.responseReceivedExtraInfo', event => {
    for (const cookie of event.blockedCookies || []) blockedCookies.push(...cookie.blockedReasons);
  });
  await untilStopped(() => protocol.send('Network.enable'));
  const seen = [];
  page.on('response', response => seen.push(response));
  await untilStopped(() => page.goto(allowedOrigin));
  const selected = await browserFetch(page, `${syncOrigin}/cookie_sync`, 'POST', {
    account: 'publisher', bidders: [fixture.bidder], gdpr: 0, coopSync: false,
  });
  assert.equal(selected.status, 200);
  assert.equal(selected.body.status, 'no_cookie');
  assert.equal(selected.body.bidder_status.length, 1);
  const syncer = selected.body.bidder_status[0];
  assert.equal(syncer.bidder, fixture.bidder);
  assert.equal(syncer.usersync.type, fixture.sync_type);
  assert.deepEqual(syncer, {
    bidder: fixture.bidder, no_cookie: true,
    usersync: { type: fixture.sync_type, url: expectedSyncUrl },
  });
  const syncUrl = new URL(syncer.usersync.url);
  assert.equal(syncUrl.hostname, fixture.endpoint_host);
  assert.equal(syncUrl.origin, fixture.endpoint_origin);
  assert.equal(syncUrl.pathname, fixture.endpoint_path);
  assert.deepEqual([...syncUrl.searchParams], [[expectedCallback, '']]);
  const callback = new URL([...syncUrl.searchParams.keys()][0]);
  assert.equal(callback.href, expectedCallback);
  assert.equal(callback.origin, syncOrigin);
  assert.equal(callback.pathname, '/setuid');
  assert.equal(callback.searchParams.get('account'), 'publisher');
  assert.equal(callback.searchParams.get('bidder'), fixture.bidder);
  assert.equal(callback.searchParams.get('uid'), fixture.user_macro);
  callback.searchParams.set('uid', syntheticUid);
  callbackTarget = callback.href;

  const post = seen.find(response => response.url() === `${syncOrigin}/cookie_sync` && response.request().method() === 'POST');
  assert.ok(post, 'browser POST response observed');
  const postHeaders = await untilStopped(() => post.allHeaders());
  assertHeaders(postHeaders, allowedOrigin);
  assert.equal(postHeaders['set-cookie'], undefined);
  const preflight = preflights.find(response => response.url === `${syncOrigin}/cookie_sync`);
  assert.ok(preflight, 'browser generated a JSON preflight');
  assert.equal(preflight.status, 204);
  assertHeaders(Object.fromEntries(Object.entries(preflight.headers).map(([name, value]) => [name.toLowerCase(), value])), allowedOrigin);

  const navigation = await untilStopped(() => page.goto(`${allowedOrigin}/callback`));
  assert.equal(navigation.status(), 200);
  assert.equal(new URL(page.url()).pathname, '/setuid');
  const navHeaders = await untilStopped(() => navigation.allHeaders());
  assert.equal(navHeaders['cache-control'], 'no-store');
  assert.ok(navHeaders['set-cookie'], 'callback emitted its cookie');
  // Playwright's URL filter excludes Secure cookies for HTTP, even on loopback.
  // Inspect storage here; the following fetch separately proves browser delivery.
  const cookies = await untilStopped(() => context.cookies());
  const uidCookie = cookies.find(cookie => cookie.name === 'uids');
  assert.ok(uidCookie, 'Chromium accepted the actual Set-Cookie response');
  assert.ok(uidCookie.value.startsWith('v1.test.'), 'browser stored the signed envelope');
  assert.ok(Buffer.byteLength(navHeaders['set-cookie']) <= 4096, 'complete cookie field fits the browser budget');
  assert.deepEqual(blockedCookies, [], 'Chromium reported no blocked Set-Cookie fields');
  assert.equal(uidCookie.domain, '127.0.0.1');
  assert.equal(uidCookie.path, '/');
  assert.equal(uidCookie.secure, true);
  assert.equal(uidCookie.sameSite, 'None');
  assert.ok(uidCookie.expires > Date.now() / 1_000);

  await untilStopped(() => page.goto(allowedOrigin));
  const uids = await browserFetch(page, `${syncOrigin}/getuids`);
  assert.equal(uids.status, 200);
  assert.equal(uids.body.buyeruids[fixture.family], syntheticUid);
  assert.deepEqual(uids.body, { buyeruids: { [fixture.family]: syntheticUid } });
  const get = seen.find(response => response.url() === `${syncOrigin}/getuids` && response.request().method() === 'GET');
  assert.ok(get, 'browser GET response observed');
  assertHeaders(await untilStopped(() => get.allHeaders()), allowedOrigin);
  const cookieHeader = await untilStopped(() => get.request().headerValue('cookie'));
  assert.ok(cookieHeader?.includes('uids='), 'browser attached cookie to credentialed fetch');

  await untilStopped(() => page.goto(deniedOrigin));
  const preflightCount = preflights.length;
  const postCount = seen.filter(response => response.request().method() === 'POST').length;
  assert.deepEqual(await browserFetch(page, `${syncOrigin}/getuids`), { error: 'TypeError' });
  assert.deepEqual(await browserFetch(page, `${syncOrigin}/cookie_sync`, 'POST', {
    account: 'publisher', bidders: [fixture.bidder], gdpr: 0,
  }), { error: 'TypeError' });
  const deniedPreflight = preflights.slice(preflightCount).find(response => response.url === `${syncOrigin}/cookie_sync`);
  assert.ok(deniedPreflight, 'browser sent a denied-origin preflight');
  assert.equal(deniedPreflight.status, 403);
  const deniedHeaders = Object.fromEntries(Object.entries(deniedPreflight.headers).map(([name, value]) => [name.toLowerCase(), value]));
  assert.equal(deniedHeaders['cache-control'], 'no-store');
  assert.equal(deniedHeaders['access-control-allow-origin'], undefined);
  assert.equal(deniedHeaders['access-control-allow-credentials'], undefined);
  assert.equal(deniedHeaders['set-cookie'], undefined);
  assert.equal(seen.filter(response => response.request().method() === 'POST').length, postCount, 'denied fetch produced no POST response');
  assert.deepEqual(await untilStopped(() => context.cookies()), cookies, 'denied-origin requests did not mutate the cookie');
  await untilStopped(() => page.goto(allowedOrigin));
  const recovered = await browserFetch(page, `${syncOrigin}/getuids`);
  assert.equal(recovered.status, 200, 'allowed-origin control rules out a server outage');
  assert.equal(recovered.body.buyeruids[fixture.family], syntheticUid);
  assert.deepEqual(recovered.body, { buyeruids: { [fixture.family]: syntheticUid } });
  const deletionQuery = new URLSearchParams({ bidder: fixture.bidder, account: 'publisher', gdpr: '0', f: 'b', uid: '' });
  const deletionUrl = `${syncOrigin}/setuid?${deletionQuery}`;
  const deletion = await browserFetch(page, deletionUrl, 'GET', undefined, false);
  assert.equal(deletion.status, 200);
  assert.equal(deletion.body, '');
  const deleteResponse = seen.find(response => response.url() === deletionUrl);
  assert.ok(deleteResponse, 'browser deletion response observed');
  const deletionHeaders = await untilStopped(() => deleteResponse.allHeaders());
  assert.ok(deletionHeaders['set-cookie'].includes('Max-Age=0'));
  assert.ok(!((await untilStopped(() => context.cookies())).some(cookie => cookie.name === 'uids')), 'browser removed UID cookie');
  const emptyReadback = await browserFetch(page, `${syncOrigin}/getuids`);
  assert.equal(emptyReadback.status, 200);
  assert.deepEqual(emptyReadback.body, {});
  checkRunning();
  evidence = {
    result: 'passed', browser: `Chromium ${browser.version()}`,
    checks: ['credentialed CORS', 'JSON preflight', 'emitted callback through controlled redirect', 'signed Secure/SameSite=None cookie acceptance', 'credentialed cookie visibility', 'denied origin', 'browser deletion and empty readback'],
    scope: 'loopback, cross-origin same-site; no real bidder or hosted provider contacted',
  };
} catch (error) {
  if (!interrupted) throw error;
} finally {
  const failed = await cleanup().catch(reason => ({ status: 'rejected', reason }));
  for (const [signal, handler] of signalHandlers) process.off(signal, handler);
  if (interrupted) {
    if (failed) console.error('Browser check cleanup did not fully complete');
    process.exitCode = exitCodes[interrupted];
  }
  else if (failed) throw failed.reason;
}

if (!interrupted) console.log(JSON.stringify(evidence, null, 2));
