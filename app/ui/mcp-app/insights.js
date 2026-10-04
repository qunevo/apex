(() => {
  const host = window.apexHost;
  const { node, number } = window.apexViews;
  const $ = id => document.getElementById(id);
  let current, expected;
  let generation = 0, busy = false, canCall = false, stopped = false, section = 'all';
  function message(text, error = false) { $('message').textContent = text; $('message').dataset.error = String(error); }
  function controls() { $('refresh').disabled = !current || !canCall || busy; $('view-select').disabled = !current || !canCall || busy; $('show-schedule').disabled = !current?.result_id || !canCall || busy; }
  function context(value) { if (['dark', 'light'].includes(value.theme)) document.documentElement.dataset.theme = value.theme; }
  function argumentsFor(data, view = data.view_id) { return { scenario_id: data.scenario_id, revision: data.revision, ...(data.result_id ? { result_id: data.result_id } : {}), view_id: view }; }
  function validate(data, args) {
    if (data?.kind !== 'apex_view' || !data.scenario_id || !Number.isInteger(data.revision) || data.revision < 1 || !data.content_hash || !data.view_id
      || !Array.isArray(data.available_views) || !data.available_views.some(v => v.id === data.view_id)
      || !Array.isArray(data.dashboard?.metrics) || !Array.isArray(data.dashboard?.panels) || !Array.isArray(data.dashboard?.notes)) throw new Error('The tool did not return an APEX dashboard');
    if (args && (data.scenario_id !== args.scenario_id || (args.revision != null && data.revision !== args.revision)
      || (data.result_id ?? null) !== (args.result_id ?? null) || (args.view_id && data.view_id !== args.view_id))) throw new Error('The response belongs to another snapshot');
    if (data.dashboard.metrics.length > 12 || data.dashboard.panels.length > 12 || data.dashboard.notes.length > 12
      || data.dashboard.metrics.some(m => m.value !== null && !Number.isFinite(m.value))) throw new Error('Invalid dashboard metrics');
    const ids = new Set();
    for (const p of data.dashboard.panels) {
      if (!p.id || ids.has(p.id) || !['input', 'result', 'orders', 'resources'].includes(p.section)) throw new Error('Invalid dashboard panel');
      ids.add(p.id);
      if (['bars', 'donut'].includes(p.kind) && (!Array.isArray(p.points) || p.points.length > 24
        || p.points.some(v => typeof v.label !== 'string' || !Number.isFinite(v.value) || v.value < 0))) throw new Error('Invalid chart data');
      if (p.kind === 'table' && (!Array.isArray(p.columns) || !Array.isArray(p.rows) || p.rows.length > 24
        || p.rows.some(r => !Array.isArray(r) || r.length !== p.columns.length))) throw new Error('Invalid table data');
      if (p.kind === 'custom' && (!data.package || !p.renderer?.startsWith(`${data.package.id}.`))) throw new Error('Renderer does not belong to this package');
    }
  }
  function filter() {
    for (const panel of $('panels').children) panel.hidden = section !== 'all' && panel.dataset.section !== section;
    for (const button of $('sections').children) button.setAttribute('aria-pressed', String(button.dataset.section === section));
  }
  function accept(response, args) {
    if (response.isError) throw new Error(response.structuredContent?.message ?? 'Unable to read this view');
    const data = response.structuredContent;
    validate(data, args);
    current = data; expected = argumentsFor(data);
    $('dashboard').hidden = false; $('schedule-view').hidden = true;
    $('show-schedule').hidden = !data.result_id;
    $('title').textContent = data.dashboard.title;
    $('subtitle').textContent = data.dashboard.subtitle;
    $('context').textContent = data.scenario_name;
    $('revision').textContent = `Revision ${data.revision}`;
    $('validation').textContent = data.result_id ? (data.validation_valid ? 'Independently validated result' : 'Result validation failed') : 'Input snapshot · no planning result';
    $('validation').dataset.invalid = String(data.result_id && !data.validation_valid);
    $('package').textContent = data.package ? `${data.package.id} @ ${data.package.version}` : 'Standard configuration';
    $('result-status').textContent = data.result_status ?? '';
    $('metrics').replaceChildren(...data.dashboard.metrics.map(metric => {
      const card = node('article', undefined, 'metric');
      const value = node('strong', metric.value === null ? '—' : number(metric.value), 'metric-value');
      value.dataset.unknown = String(metric.value === null);
      if (metric.value !== null) value.append(node('small', metric.unit));
      card.append(node('span', metric.label, 'metric-label'), value, node('span', metric.detail, 'metric-detail')); return card;
    }));
    $('panels').replaceChildren(...data.dashboard.panels.map(p => {
      const panel = node('section', undefined, 'panel'); panel.dataset.kind = p.kind; panel.dataset.section = p.section; panel.dataset.panel = p.id; panel.setAttribute('aria-label', p.title);
      panel.append(node('h2', p.title), node('p', p.description, 'panel-description'));
      const body = node('div'); window.apexViews.render(body, p); panel.append(body); return panel;
    }));
    if (!data.dashboard.panels.length) $('panels').append(node('p', 'No chart data is available for this snapshot yet.', 'empty'));
    for (const button of $('sections').children) button.disabled = button.dataset.section !== 'all' && !data.dashboard.panels.some(p => p.section === button.dataset.section);
    if (section !== 'all' && !data.dashboard.panels.some(p => p.section === section)) section = 'all';
    $('view-select').replaceChildren(...data.available_views.map(v => new Option(v.title, v.id))); $('view-select').value = data.view_id;
    $('notes').replaceChildren(...data.dashboard.notes.map(n => node('p', n)));
    $('identity').textContent = `Scenario ${data.scenario_id} · Revision ${data.revision} · Content ${data.content_hash}${data.result_id ? ` · Result ${data.result_id}` : ''}`;
    $('sources').replaceChildren(...(data.source_summary?.sources ?? []).map(s => node('p', `${s.label}${s.revision ? ` · revision ${s.revision}` : ''} · SHA256 ${s.sha256}`)));
    $('main').hidden = false; filter(); controls();
    message('Saved planning snapshot. Refresh keeps the selected revision.');
  }
  async function retrieve(view) {
    if (!current || !canCall || busy || stopped) return;
    const args = argumentsFor(current, view); const version = ++generation;
    busy = true; controls(); message('Refreshing the saved snapshot…');
    try { const response = await host.request('tools/call', { name: 'views.get', arguments: args }); if (!stopped && version === generation) accept(response, args); }
    catch (error) { if (!stopped && version === generation) { $('view-select').value = current.view_id; message(`Refresh failed; the saved view remains visible. ${error.message}`, true); } }
    finally { busy = false; controls(); }
  }
  host.on('ui/notifications/tool-input', ({ arguments: args }) => {
    if (stopped) return;
    generation++; current = undefined; expected = args; section = 'all'; $('main').hidden = true; controls(); message('Loading snapshot…');
  });
  host.on('ui/notifications/tool-result', response => {
    if (stopped) return;
    generation++;
    try { accept(response, expected); } catch (error) { current = undefined; $('main').hidden = true; controls(); message(error.message, true); }
  });
  host.on('ui/notifications/tool-cancelled', value => { generation++; current = undefined; $('main').hidden = true; controls(); message(value.reason ?? 'The tool call was cancelled', true); });
  host.on('ui/notifications/host-context-changed', context);
  $('refresh').addEventListener('click', () => retrieve(current?.view_id));
  $('view-select').addEventListener('change', () => retrieve($('view-select').value));
  $('back-overview').addEventListener('click', () => { $('dashboard').hidden = false; $('schedule-view').hidden = true; });
  $('show-schedule').addEventListener('click', async () => {
    if (!current?.result_id || !canCall || busy || stopped) return;
    const snapshot = current; const version = ++generation; busy = true; controls(); message('Opening saved schedule…');
    try {
      const response = await host.request('tools/call', { name: 'results.get', arguments: { result_id: snapshot.result_id } });
      if (version !== generation || stopped) return;
      const data = response.structuredContent;
      if (response.isError) throw new Error(data?.message ?? 'Unable to read the schedule');
      if (data?.id !== snapshot.result_id || data.scenario !== snapshot.scenario_id || data.revision !== snapshot.revision || data.provenance?.content_hash !== snapshot.content_hash) throw new Error('The schedule belongs to another snapshot');
      if (!Array.isArray(data.view?.operations) || data.view.operations.some(op => typeof op.id !== 'string' || typeof op.resource !== 'string' || !Number.isFinite(op.start) || !Number.isFinite(op.end) || op.end < op.start)) throw new Error('Invalid schedule view');
      window.apexSchedule.render($('schedule-content'), data); $('dashboard').hidden = true; $('schedule-view').hidden = false;
      message('Showing the saved schedule for this revision.');
    } catch (error) { if (version === generation && !stopped) message(error.message, true); }
    finally { busy = false; controls(); }
  });
  for (const button of $('sections').children) button.addEventListener('click', () => { section = button.dataset.section; filter(); });
  const observer = new ResizeObserver(() => host.notify('ui/notifications/size-changed', { width: document.documentElement.clientWidth, height: document.body.scrollHeight }));
  host.on('teardown', () => { stopped = true; generation++; canCall = false; observer.disconnect(); controls(); });
  host.connect().then(info => {
    if (stopped) return;
    context(info.hostContext ?? {}); canCall = !!info.hostCapabilities?.serverTools;
    host.notify('ui/notifications/initialized'); observer.observe(document.body); controls();
    if (!current) message('Waiting for a scenario snapshot…');
  }).catch(error => message(error.message, true));
})();
