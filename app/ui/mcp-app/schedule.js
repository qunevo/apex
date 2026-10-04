// The shared main-operation view. Outcome metrics use the server's product-ready events.
window.apexSchedule = Object.freeze({
  render(container, data) {
    const { node, table } = window.apexViews;
    const all = data.view.operations;
    const controls = node('div', undefined, 'schedule-controls');
    const search = node('input'); search.type = 'search'; search.placeholder = 'Find an operation…'; search.setAttribute('aria-label', 'Find an operation');
    const resource = node('select'); resource.setAttribute('aria-label', 'Schedule resource');
    resource.append(new Option('All resources', ''), ...[...new Set(all.map(op => op.resource))].sort().map(id => new Option(id, id)));
    const count = node('span', '', 'chart-detail'); controls.append(search, resource, count);
    const chart = node('div', undefined, 'schedule-chart');
    const details = node('details'); details.append(node('summary', 'Operation details')); const rows = node('div'); details.append(rows);
    const start = all.reduce((min, op) => Math.min(min, op.start), all[0]?.start ?? 0);
    const end = all.reduce((max, op) => Math.max(max, op.end), start + 1);
    const span = Math.max(1, end - start);
    function time(seconds) {
      const at = data.view.epoch && Date.parse(data.view.epoch) + seconds * 1000;
      return Number.isFinite(at) ? `${new Date(at).toISOString().slice(5, 16).replace('T', ' ')} UTC` : `${(seconds / 3600).toFixed(1)} h`;
    }
    function draw() {
      const filtered = all.filter(op => (!resource.value || op.resource === resource.value) && `${op.id} ${op.label ?? ''}`.toLowerCase().includes(search.value.toLowerCase()));
      const shown = filtered.toSorted((a, b) => a.start - b.start).slice(0, 200);
      count.textContent = `${shown.length} shown · ${filtered.length} matching · ${all.length} total`;
      chart.replaceChildren();
      const axis = node('div', undefined, 'schedule-row'); axis.append(node('span', 'Resource', 'schedule-label'));
      const ticks = node('div', undefined, 'schedule-track schedule-axis');
      for (const ratio of [0, .5, 1]) ticks.append(node('span', time(start + span * ratio)));
      axis.append(ticks); chart.append(axis);
      const groups = new Map();
      for (const op of shown) { if (!groups.has(op.resource)) groups.set(op.resource, []); groups.get(op.resource).push(op); }
      for (const [id, operations] of groups) {
        const row = node('div', undefined, 'schedule-row'); row.append(node('span', id, 'schedule-label'));
        const track = node('div', undefined, 'schedule-track'); const lanes = [];
        for (const op of operations) {
          let lane = lanes.findIndex(end => end <= op.start); if (lane < 0) lane = lanes.length; lanes[lane] = op.end;
          const bar = node('span', op.id, 'schedule-bar');
          bar.style.left = `${(op.start - start) / span * 100}%`; bar.style.width = `${(op.end - op.start) / span * 100}%`; bar.style.top = `${lane * 28}px`;
          bar.title = `${op.id} · ${op.label ?? ''} · ${time(op.start)} – ${time(op.end)}`; track.append(bar);
        }
        track.style.height = `${Math.max(1, lanes.length) * 28}px`; row.append(track); chart.append(row);
      }
      if (!shown.length) chart.append(node('p', 'No operations match these filters.', 'empty'));
      rows.replaceChildren(table(['Operation', 'Resource', 'Start', 'Main end'], shown.map(op => [op.id, op.resource, time(op.start), time(op.end)])));
    }
    controls.addEventListener('input', draw); resource.addEventListener('change', draw);
    container.replaceChildren(node('h2', 'Production schedule'), node('p', 'Main operations on their primary resources. Filter to inspect up to 200 operations at a time. Product-ready completion can include later post-processing.', 'panel-description'), controls, chart, details);
    draw();
  },
});
