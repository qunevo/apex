(() => {
  const host = window.apexHost;
  const $ = id => document.getElementById(id);
  const main = document.querySelector('main');
  let current;
  let expectedId;
  let canRefresh = false;
  let busy = false;
  let generation = 0;
  function node(tag, text, className) {
    const element = document.createElement(tag);
    if (text !== undefined) element.textContent = text;
    if (className) element.className = className;
    return element;
  }
  function message(text, error = false) { $('message').textContent = text; $('message').dataset.error = String(error); }
  function refreshState() { $('refresh').disabled = !current || !canRefresh || busy; }
  function context(value) {
    if (value.theme === 'dark' || value.theme === 'light') document.documentElement.dataset.theme = value.theme;
  }
  function formatTime(seconds) {
    if (current.view.epoch) {
      const timestamp = Date.parse(current.view.epoch) + seconds * 1000;
      if (Number.isFinite(timestamp)) return new Date(timestamp).toISOString().replace('T', ' ').replace('.000Z', ' UTC');
    }
    return `${seconds.toLocaleString('en-US')} s`;
  }
  function draw() {
    const query = $('search').value.toLowerCase();
    const resource = $('resource').value;
    const all = current.view.operations;
    const shown = all.filter(op => (!resource || op.resource === resource) && `${op.id} ${op.label ?? ''}`.toLowerCase().includes(query));
    $('count').textContent = `${shown.length} of ${all.length} operations`;
    const start = all.reduce((min, op) => Math.min(min, op.start), 0);
    const end = all.reduce((max, op) => Math.max(max, op.end), start + 1);
    const span = Math.max(1, end - start);
    $('timebase').textContent = current.view.epoch ? `Time origin: ${current.view.epoch} · dates shown in UTC` : 'Time in seconds relative to the planning origin';
    const chart = $('chart'); chart.replaceChildren();
    const axis = node('div', undefined, 'chart-row');
    axis.append(node('span', 'Resource', 'resource-label'));
    const ticks = node('div', undefined, 'axis track');
    for (const ratio of [0, .5, 1]) ticks.append(node('span', formatTime(start + Math.round(span * ratio))));
    axis.append(ticks); chart.append(axis);
    const groups = new Map();
    for (const op of shown) { if (!groups.has(op.resource)) groups.set(op.resource, []); groups.get(op.resource).push(op); }
    for (const [name, operations] of groups) {
      const row = node('div', undefined, 'chart-row');
      row.append(node('div', name, 'resource-label'));
      const track = node('div', undefined, 'track');
      const lanes = [];
      for (const op of operations.toSorted((a, b) => a.start - b.start)) {
        let lane = lanes.findIndex(end => end <= op.start);
        if (lane < 0) lane = lanes.length;
        lanes[lane] = op.end;
        const bar = node('span', op.label ?? op.id, 'bar');
        bar.style.left = `${(op.start - start) / span * 100}%`;
        bar.style.width = `${Math.max(0, op.end - op.start) / span * 100}%`;
        bar.style.top = `${lane * 29}px`;
        bar.title = `${op.id} · ${name} · ${formatTime(op.start)} – ${formatTime(op.end)}`;
        track.append(bar);
      }
      track.style.height = `${Math.max(1, lanes.length) * 29}px`;
      row.append(track); chart.append(row);
    }
    if (!shown.length) chart.append(node('p', 'No operations match these filters.', 'resource-label'));
    const rows = shown.map(op => {
      const row = node('tr');
      for (const value of [op.label ? `${op.id} · ${op.label}` : op.id, op.resource, formatTime(op.start), formatTime(op.end)]) row.append(node('td', value));
      return row;
    });
    $('operations').replaceChildren(...rows);
  }
  function accept(result) {
    if (result.isError) throw new Error(result.structuredContent?.message ?? result.content?.find(c => c.type === 'text')?.text ?? 'Unable to read the result');
    const data = result.structuredContent;
    if (!data?.id || !data.view || !Array.isArray(data.view.operations) || !data.provenance || !data.validation) throw new Error('The tool did not return an APEX planning result');
    if (expectedId && data.id !== expectedId) throw new Error('The response belongs to another result');
    if (data.view.operations.some(op => typeof op.id !== 'string' || typeof op.resource !== 'string' || !Number.isFinite(op.start) || !Number.isFinite(op.end) || op.end < op.start)) throw new Error('Invalid schedule view');
    const previousResource = $('resource').value;
    current = data;
    expectedId = data.id;
    $('status').textContent = data.status;
    $('revision').textContent = `Revision ${data.revision}`;
    $('validation').textContent = data.validation.valid ? 'Independently validated' : 'Validation failed';
    const pkg = data.provenance.customization_package;
    $('package').textContent = pkg ? `${pkg.id} @ ${pkg.version}` : 'Standard configuration';
    $('identity').textContent = `Scenario ${data.scenario} · Result ${data.id} · Engine ${data.provenance.engine.id} @ ${data.provenance.engine.version} · Content ${data.provenance.content_hash}`;
    $('metrics').replaceChildren(...Object.entries(data.metrics ?? {}).map(([key, value]) => {
      const card = node('div', undefined, 'metric');
      card.append(node('span', key.replaceAll('_', ' ')), node('strong', Number(value).toLocaleString('en-US', { maximumFractionDigits: 2 })));
      return card;
    }));
    const options = [new Option('All resources', ''), ...[...new Set(data.view.operations.map(op => op.resource))].sort().map(id => new Option(id, id))];
    $('resource').replaceChildren(...options);
    if (options.some(option => option.value === previousResource)) $('resource').value = previousResource;
    main.hidden = false; draw(); refreshState();
    message('Showing the saved result. Refresh to retrieve its current approval status.');
  }
  host.on('ui/notifications/tool-input', ({ arguments: args }) => {
    generation++; current = undefined; expectedId = args?.result_id; main.hidden = true; refreshState(); message('Loading result…');
  });
  host.on('ui/notifications/tool-result', result => {
    generation++;
    try { accept(result); } catch (error) { current = undefined; main.hidden = true; refreshState(); message(error.message, true); }
  });
  host.on('ui/notifications/tool-cancelled', value => {
    generation++; current = undefined; main.hidden = true; refreshState(); message(value.reason ?? 'The tool call was cancelled', true);
  });
  host.on('ui/notifications/host-context-changed', context);
  $('search').addEventListener('input', () => { if (current) draw(); });
  $('resource').addEventListener('change', () => { if (current) draw(); });
  $('refresh').addEventListener('click', async () => {
    if (!current || busy || !canRefresh) return;
    const version = generation;
    busy = true; refreshState(); message('Refreshing result…');
    try {
      const result = await host.request('tools/call', { name: 'results.get', arguments: { result_id: current.id } });
      if (version === generation) accept(result);
    } catch (error) { if (version === generation) message(`Refresh failed; the saved view remains visible. ${error.message}`, true); }
    finally { busy = false; refreshState(); }
  });
  const observer = new ResizeObserver(() => host.notify('ui/notifications/size-changed', { width: document.documentElement.clientWidth, height: document.body.scrollHeight }));
  host.on('teardown', () => { generation++; canRefresh = false; observer.disconnect(); refreshState(); });
  host.connect().then(info => {
    context(info.hostContext ?? {}); canRefresh = !!info.hostCapabilities?.serverTools;
    host.notify('ui/notifications/initialized'); observer.observe(document.body);
    refreshState(); message('Waiting for a planning result…');
  }).catch(error => message(error.message, true));
})();
