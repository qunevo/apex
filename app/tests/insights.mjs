// Run test:control first: these documents and HTML come from the release binary.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { chromium } from 'playwright';

const reports = new URL('../.apex/reports/', import.meta.url);
const html = await readFile(new URL('insights.html', reports), 'utf8');
const load = async name => JSON.parse(await readFile(new URL(`insights-${name}.json`, reports), 'utf8'));
const [input, result, sources, resultSources, schedule] = await Promise.all(['input', 'result', 'sources', 'result-sources', 'schedule'].map(load));
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1160, height: 1500 } });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.setContent('<!doctype html><title>APEX synthetic showcase host</title><style>body{margin:0}iframe{width:100%;height:1480px;border:0}</style><iframe title="APEX Planning overview" sandbox="allow-scripts"></iframe>');
  await page.evaluate(({ sources, resultSources, schedule }) => {
    window.requests = []; window.ready = false; window.schedule = schedule;
    const iframe = document.querySelector('iframe');
    window.send = (method, params) => iframe.contentWindow.postMessage({ jsonrpc: '2.0', method, params }, '*');
    window.addEventListener('message', event => {
      if (event.source !== iframe.contentWindow) return;
      const m = event.data; window.requests.push(m);
      const reply = result => event.source.postMessage({ jsonrpc: '2.0', id: m.id, result }, '*');
      if (m.method === 'ui/initialize') reply({ protocolVersion: '2026-01-26', hostCapabilities: { serverTools: {} }, hostContext: { theme: 'light' } });
      else if (m.method === 'ui/notifications/initialized') window.ready = true;
      else if (m.method === 'ui/notifications/size-changed') iframe.style.height = `${Math.max(500, m.params.height)}px`;
      else if (m.method === 'tools/call') {
        const document = m.params.name === 'results.get' ? window.schedule
          : m.params.arguments.view_id === 'demo.sources' ? (m.params.arguments.result_id ? resultSources : sources) : window.next;
        const response = window.fail ? { isError: true, structuredContent: { message: 'Permission denied' } } : { structuredContent: document };
        if (window.delay) window.pendingReply = () => reply(response); else reply(response);
      }
    });
  }, { sources, resultSources, schedule });
  await page.locator('iframe').evaluate((frame, text) => { frame.srcdoc = text; }, html);
  await page.waitForFunction(() => window.ready);
  const frame = page.frameLocator('iframe');
  async function screenshot(name) {
    const height = await frame.locator('body').evaluate(el => el.scrollHeight);
    await page.waitForFunction(height => document.querySelector('iframe').clientHeight >= height, height);
    await page.screenshot({ path: new URL(name, reports).pathname.replace(/^\/(\w:)/, '$1'), fullPage: true });
  }
  async function show(data, args = { scenario_id: data.scenario_id, revision: data.revision, ...(data.result_id ? { result_id: data.result_id } : {}), view_id: data.view_id }) {
    await page.evaluate(({ data, args }) => {
      window.next = data;
      window.send('ui/notifications/tool-input', { arguments: args });
      window.send('ui/notifications/tool-result', { structuredContent: data });
    }, { data, args });
  }
  await show(input);
  await frame.locator('main').waitFor({ state: 'visible' });
  assert.equal(await frame.locator('h1').textContent(), 'Planning overview');
  assert.match(await frame.locator('.apex-brand').textContent(), /by Qunevo/);
  assert.equal(await frame.locator('.wordmark svg path').count(), 5);
  assert.equal(await frame.locator('.metric-value[data-unknown="true"]').count(), 3);
  assert.equal(await frame.locator('#show-schedule').isVisible(), false);
  assert.equal(await frame.locator('#sections [data-section="result"]').isEnabled(), false);
  assert.match(await frame.locator('[data-panel="critical-orders"]').textContent(), /Upcoming orders/);
  const mix = frame.locator('[data-panel="work-mix"]');
  await mix.locator('.chart-bar').first().click();
  assert.match(await mix.locator('.chart-detail').textContent(), /33.3% of all groups/);
  await mix.locator('input').fill('Machining');
  assert.equal(await mix.locator('.chart-bar').count(), 1);
  await mix.getByRole('button', { name: 'Data table' }).click();
  assert.equal(await mix.locator('tbody tr').count(), 1);
  await mix.locator('input').fill('absent');
  assert.match(await mix.textContent(), /No groups match/);
  await frame.locator('#refresh').click();
  await frame.locator('#message').filter({ hasText: 'Saved planning snapshot' }).waitFor();
  assert.deepEqual(await page.evaluate(() => window.requests.find(r => r.method === 'tools/call').params), {
    name: 'views.get', arguments: { scenario_id: input.scenario_id, revision: input.revision, view_id: 'overview' },
  });
  await frame.locator('#view-select').selectOption('demo.sources');
  await frame.locator('h1').filter({ hasText: 'Source details' }).waitFor();
  assert.match(await frame.locator('[data-panel="source-flow"]').textContent(), /32 matched/);
  assert.equal(await frame.locator('[data-panel="work-mix"]').count(), 0);
  await frame.locator('#view-select').selectOption('overview');
  await frame.locator('h1').filter({ hasText: 'Planning overview' }).waitFor();
  await page.evaluate(() => { window.fail = true; });
  await frame.locator('#refresh').click();
  await frame.locator('#message').filter({ hasText: 'Permission denied' }).waitFor();
  assert.equal(await frame.locator('main').isVisible(), true);
  await page.evaluate(() => { window.fail = false; });
  await show(result);
  await frame.locator('#validation').filter({ hasText: 'Independently validated result' }).waitFor();
  assert.match(await frame.locator('[data-panel="critical-orders"] tbody').textContent(), /Late/);
  await frame.locator('#sections [data-section="resources"]').click();
  assert.equal(await mix.isVisible(), false);
  const load = frame.locator('[data-panel="resource-load"]');
  assert.equal(await load.isVisible(), true);
  await load.locator('.chart-bar').first().click();
  assert.doesNotMatch(await load.locator('.chart-detail').textContent(), /of all groups|total/);
  await load.getByRole('button', { name: 'Data table' }).click();
  assert.equal(await load.locator('th').count(), 2, 'Resource ratios must not be summed');
  await load.getByRole('button', { name: 'Chart', exact: true }).click();
  await frame.locator('#sections [data-section="all"]').click();
  await frame.locator('[data-panel="delivery-outlook"] .legend button').first().click();
  assert.match(await frame.locator('[data-panel="delivery-outlook"] .chart-detail').textContent(), /% of all groups/);
  await screenshot('planning-overview-light.png');
  await page.evaluate(() => window.send('ui/notifications/host-context-changed', { theme: 'dark' }));
  await frame.locator('html[data-theme="dark"]').waitFor();
  await screenshot('planning-overview-dark.png');
  await page.evaluate(() => window.send('ui/notifications/host-context-changed', { theme: 'light' }));
  for (const width of [320, 390, 768, 1160]) {
    await page.setViewportSize({ width, height: 844 });
    assert.equal(await frame.locator('body').evaluate(el => el.scrollWidth <= document.documentElement.clientWidth), true, `No horizontal overflow at ${width}px`);
    if (width === 390) await screenshot('planning-overview-mobile.png');
  }

  await frame.locator('#show-schedule').click();
  await frame.locator('#schedule-view').waitFor({ state: 'visible' });
  assert.equal(await frame.locator('.schedule-bar').count(), 36);
  await frame.getByRole('combobox', { name: 'Schedule resource' }).selectOption('CNC-01');
  const resourceCount = schedule.view.operations.filter(op => op.resource === 'CNC-01').length;
  assert.equal(await frame.locator('.schedule-bar').count(), resourceCount);
  await frame.getByRole('searchbox', { name: 'Find an operation' }).fill('absent');
  assert.match(await frame.locator('.schedule-chart').textContent(), /No operations match/);
  await frame.getByRole('searchbox', { name: 'Find an operation' }).fill('');
  await frame.getByRole('combobox', { name: 'Schedule resource' }).selectOption('');
  await screenshot('planning-overview-schedule.png');
  await frame.locator('#back-overview').click();
  await page.evaluate(() => { window.schedule = { ...window.schedule, revision: 99 }; });
  await frame.locator('#show-schedule').click();
  await frame.locator('#message').filter({ hasText: 'another snapshot' }).waitFor();
  assert.equal(await frame.locator('#dashboard').isVisible(), true);
  // The schedule stays bounded, but filters can still find records beyond the first 200.
  const large = structuredClone(schedule);
  large.view.operations = Array.from({ length: 300 }, (_, i) => ({ id: `EXTRA-${i}`, resource: 'R', start: i * 60, end: i * 60 + 30 }));
  await page.evaluate(schedule => { window.schedule = schedule; }, large);
  await frame.locator('#show-schedule').click();
  await frame.locator('#schedule-view').waitFor({ state: 'visible' });
  assert.equal(await frame.locator('.schedule-bar').count(), 200);
  await frame.getByRole('searchbox', { name: 'Find an operation' }).fill('EXTRA-250');
  assert.equal(await frame.locator('.schedule-bar').count(), 1);
  assert.match(await frame.locator('.schedule-bar').textContent(), /EXTRA-250/);
  await frame.locator('#back-overview').click();
  await page.evaluate(schedule => { window.schedule = schedule; }, schedule);

  const malicious = structuredClone(sources);
  malicious.dashboard.title = '<img src=x onerror=alert(1)>';
  malicious.dashboard.panels[0].renderer = 'demo.unavailable';
  await show(malicious);
  await frame.locator('h1').filter({ hasText: '<img src=x' }).waitFor();
  assert.equal(await frame.locator('img').count(), 0);
  assert.match(await frame.locator('[data-panel="source-flow"]').textContent(), /renderer that is not installed/);
  const empty = structuredClone(input);
  empty.dashboard.panels = [{ id: 'empty', title: 'Empty', description: '', section: 'input', kind: 'donut', unit: 'operations', points: [] }];
  await show(empty);
  await frame.locator('.empty').filter({ hasText: 'No groups' }).waitFor();
  const invalid = structuredClone(empty); invalid.dashboard.panels[0].points = [{ label: 'Invalid', value: -1 }];
  await show(invalid);
  await frame.locator('#message').filter({ hasText: 'Invalid chart data' }).waitFor();
  assert.equal(await frame.locator('main').isVisible(), false);
  await show(input, { scenario_id: 'wrong-scenario' });
  await frame.locator('#message').filter({ hasText: 'another snapshot' }).waitFor();
  await show(input, { scenario_id: input.scenario_id, revision: 99 });
  await frame.locator('#message').filter({ hasText: 'another snapshot' }).waitFor();

  // A late refresh must never overwrite a newly selected result.
  await show(input); await frame.locator('main').waitFor({ state: 'visible' });
  await page.evaluate(() => { window.delay = true; });
  await frame.locator('#refresh').click();
  await page.waitForFunction(() => typeof window.pendingReply === 'function');
  await show(result);
  await frame.locator('#validation').filter({ hasText: 'Independently validated result' }).waitFor();
  await page.evaluate(() => { window.pendingReply(); window.delay = false; });
  assert.match(await frame.locator('#validation').textContent(), /Independently validated result/);
  await page.evaluate(() => window.send('ui/notifications/tool-cancelled', { reason: 'Cancelled by host' }));
  await frame.locator('#message').filter({ hasText: 'Cancelled by host' }).waitFor();
  assert.equal(await frame.locator('#show-schedule').isEnabled(), false);
  await page.evaluate(() => document.querySelector('iframe').contentWindow.postMessage({ jsonrpc: '2.0', id: 'teardown', method: 'ui/resource-teardown', params: {} }, '*'));
  await page.waitForFunction(() => window.requests.some(m => m.id === 'teardown' && m.result));
  assert.deepEqual(errors, []);

  // Portable preview with a read-only host stub backed exclusively by synthetic saved fixtures.
  const safe = value => JSON.stringify(value).replaceAll('<', '\\u003c');
  const preview = `<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>APEX Planning overview · synthetic preview</title><style>body{margin:0;background:#f5f3ed}iframe{width:100%;border:0;height:1600px}.preview{font:12px Arial;padding:10px 28px;background:#e9e6df;color:#60625c;margin:0}</style><p class="preview">Synthetic showcase preview · saved example, no live source connection</p><iframe title="APEX Planning overview" sandbox="allow-scripts"></iframe><script>const html=${safe(html)},data=${safe(result)},sources=${safe(resultSources)},schedule=${safe(schedule)};const frame=document.querySelector('iframe');addEventListener('message',e=>{if(e.source!==frame.contentWindow)return;const m=e.data;const send=(method,params)=>e.source.postMessage({jsonrpc:'2.0',method,params},'*');const reply=result=>e.source.postMessage({jsonrpc:'2.0',id:m.id,result},'*');if(m.method==='ui/initialize')reply({protocolVersion:'2026-01-26',hostCapabilities:{serverTools:{}},hostContext:{theme:'light'}});if(m.method==='ui/notifications/initialized'){send('ui/notifications/tool-input',{arguments:{scenario_id:data.scenario_id,result_id:data.result_id}});send('ui/notifications/tool-result',{structuredContent:data});}if(m.method==='tools/call')reply({structuredContent:m.params.name==='results.get'?schedule:m.params.arguments.view_id==='demo.sources'?sources:data});if(m.method==='ui/notifications/size-changed')frame.style.height=m.params.height+'px';});frame.srcdoc=html;</script></html>`;
  await writeFile(new URL('planning-overview-preview.html', reports), preview);
  console.log('Insights browser: standard overview, null outcomes, branding, charts, filters, source extensions, schedule navigation, revision pinning, errors, escaping, stale refresh, themes, 320–1160px layouts and teardown passed');
} finally { await browser.close(); }
