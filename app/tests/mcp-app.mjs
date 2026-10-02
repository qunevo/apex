// Exercise the shipped MCP App in an opaque sandbox with a protocol host.
import assert from 'node:assert/strict';
import { readFile, mkdir } from 'node:fs/promises';
import { chromium } from 'playwright';

let html = await readFile(new URL('../ui/mcp-app/index.html', import.meta.url), 'utf8');
for (const [token, file] of [['STYLE', 'style.css'], ['BRIDGE', 'bridge.js'], ['VIEW', 'view.js']]) {
  html = html.replace(`/* APEX_APP_${token} */`, await readFile(new URL(`../ui/mcp-app/${file}`, import.meta.url), 'utf8'));
}
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1100, height: 820 } });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.setContent('<!doctype html><title>MCP Apps test host</title><iframe title="APEX Plan" sandbox="allow-scripts" style="border:0;width:100%;height:780px"></iframe>');
  await page.evaluate(() => {
    window.requests = [];
    window.ready = false;
    window.sendToApp = (method, params) => document.querySelector('iframe').contentWindow.postMessage({ jsonrpc: '2.0', method, params }, '*');
    window.addEventListener('message', event => {
      if (event.source !== document.querySelector('iframe').contentWindow) return;
      const m = event.data;
      window.requests.push(m);
      if (m.method === 'ui/initialize') {
        window.initialization = m.params;
        event.source.postMessage({ jsonrpc: '2.0', id: m.id, result: { protocolVersion: '2026-01-26', hostInfo: { name: 'Test host', version: '1' }, hostCapabilities: { serverTools: {} }, hostContext: { theme: 'light' } } }, '*');
      } else if (m.method === 'ui/notifications/initialized') window.ready = true;
      else if (m.method === 'tools/call') {
        const result = window.failRefresh ? { isError: true, structuredContent: { message: 'Permission denied' } } : window.nextResult;
        event.source.postMessage({ jsonrpc: '2.0', id: m.id, result }, '*');
      }
    });
  });
  await page.locator('iframe').evaluate((frame, content) => { frame.srcdoc = content; }, html);
  await page.waitForFunction(() => window.ready);
  const frame = page.frameLocator('iframe');
  const result = {
    id: 'synthetic-result', scenario: 'synthetic-scenario', revision: 3, status: 'proposed',
    validation: { valid: true }, metrics: { makespan: 3600, tardiness: 0 },
    provenance: { engine: { id: 'apex', version: '1.0.0' }, content_hash: 'synthetic-hash', customization_package: { id: 'demo', version: '1' } },
    view: { resources: [{ id: 'LINE-1' }, { id: 'ASSEMBLY-1' }], operations: [
      { id: 'LOT-001:FORM', resource: 'LINE-1', start: 0, end: 1800, label: 'Form valve body' },
      { id: 'LOT-002:FORM', resource: 'LINE-1', start: 1800, end: 3000, label: 'Form valve body' },
      { id: 'LOT-001:ASSEMBLE', resource: 'ASSEMBLY-1', start: 2100, end: 3600, label: '<img src=x onerror=alert(1)>' },
    ] },
  };
  await page.evaluate(data => {
    window.nextResult = { structuredContent: data, content: [{ type: 'text', text: JSON.stringify(data) }] };
    window.sendToApp('ui/notifications/tool-input', { arguments: { result_id: data.id } });
    window.sendToApp('ui/notifications/tool-result', window.nextResult);
  }, result);
  await frame.locator('main').waitFor({ state: 'visible' });
  assert.match(await frame.locator('#count').textContent(), /3 of 3/);
  assert.equal(await frame.locator('img').count(), 0, 'Result strings must not become HTML');
  assert.equal(await frame.locator('#package').textContent(), 'demo @ 1');
  assert.equal((await page.evaluate(() => window.initialization)).appInfo.name, 'APEX Plan');
  await mkdir(new URL('../.apex/reports/', import.meta.url), { recursive: true });
  await page.screenshot({ path: new URL('../.apex/reports/mcp-app.png', import.meta.url).pathname.replace(/^\/(\w:)/, '$1') });
  await frame.locator('#resource').selectOption('LINE-1');
  assert.match(await frame.locator('#count').textContent(), /2 of 3/);
  await frame.locator('#search').fill('LOT-002');
  assert.match(await frame.locator('#count').textContent(), /1 of 3/);
  await page.evaluate(() => { window.nextResult.structuredContent.status = 'published'; });
  await frame.locator('#refresh').click();
  await frame.locator('#status').filter({ hasText: 'published' }).waitFor();
  assert.deepEqual(await page.evaluate(() => window.requests.find(r => r.method === 'tools/call').params), { name: 'results.get', arguments: { result_id: result.id } });
  await page.evaluate(() => { window.failRefresh = true; });
  await frame.locator('#refresh').click();
  await frame.locator('#message').filter({ hasText: 'Permission denied' }).waitFor();
  assert.equal(await frame.locator('main').isVisible(), true, 'Failed refresh preserves the saved view');
  await page.evaluate(() => window.sendToApp('ui/notifications/host-context-changed', { theme: 'dark' }));
  await frame.locator('html[data-theme="dark"]').waitFor();
  await page.evaluate(() => window.sendToApp('ui/notifications/tool-input', { arguments: { result_id: 'another-result' } }));
  await page.evaluate(() => window.sendToApp('ui/notifications/tool-result', window.nextResult));
  await frame.locator('#message').filter({ hasText: 'another result' }).waitFor();
  assert.equal(await frame.locator('main').isVisible(), false);
  await page.evaluate(() => window.sendToApp('ui/notifications/tool-cancelled', { reason: 'Cancelled by host' }));
  await frame.locator('#message').filter({ hasText: 'Cancelled by host' }).waitFor();
  await page.evaluate(() => document.querySelector('iframe').contentWindow.postMessage({ jsonrpc: '2.0', id: 'teardown', method: 'ui/resource-teardown', params: {} }, '*'));
  await page.waitForFunction(() => window.requests.some(m => m.id === 'teardown' && m.result));
  assert.deepEqual(errors, []);
  console.log('MCP App: handshake, result display, escaping, filters, refresh, error, identity, theme and teardown passed');
} finally { await browser.close(); }
