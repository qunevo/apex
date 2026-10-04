// Explicit renderer registry. Data is always text; package code is bundled at build time.
(() => {
  const renderers = new Map();
  const palette = ['#48756b', '#d9430b', '#b39152', '#737971', '#ab5a46', '#93897c'];
  const number = value => Number(value).toLocaleString('en-US', { maximumFractionDigits: 1 });
  function node(tag, text, className) {
    const el = document.createElement(tag);
    if (text !== undefined) el.textContent = text;
    if (className) el.className = className;
    return el;
  }
  function table(columns, rows) {
    const wrap = node('div', undefined, 'table-scroll');
    const grid = node('table'); const head = node('thead'); const headings = node('tr');
    for (const column of columns) { const th = node('th', column); th.scope = 'col'; headings.append(th); }
    head.append(headings); grid.append(head);
    const body = node('tbody');
    for (const row of rows) {
      const tr = node('tr');
      row.forEach((value, i) => { const cell = node('td', value); cell.dataset.label = columns[i]; tr.append(cell); });
      body.append(tr);
    }
    grid.append(body); wrap.append(grid);
    if (!rows.length) wrap.append(node('p', 'No records to show for this selection.', 'empty'));
    return wrap;
  }
  function charts(container, panel) {
    const points = panel.points;
    const additive = panel.unit !== '%';
    const total = points.reduce((sum, p) => sum + p.value, 0);
    const controls = node('div', undefined, 'panel-controls');
    const search = node('input'); search.type = 'search'; search.placeholder = 'Filter groups…'; search.setAttribute('aria-label', `Filter ${panel.title}`);
    const toggle = node('button', 'Data table'); toggle.setAttribute('aria-pressed', 'false');
    controls.append(search, toggle);
    const visual = node('div');
    const detail = node('p', `${number(total)} ${panel.unit} across ${points.length} groups`, 'chart-detail'); detail.setAttribute('role', 'status');
    let asTable = false;
    function select(point, button) {
      for (const b of visual.querySelectorAll('button')) b.setAttribute('aria-pressed', String(b === button));
      detail.textContent = `${point.label}: ${number(point.value)} ${panel.unit}${additive ? ` · ${total ? number(point.value / total * 100) : '0'}% of all groups` : ''}`;
    }
    function draw() {
      const shown = points.filter(p => p.label.toLowerCase().includes(search.value.toLowerCase()));
      visual.replaceChildren();
      detail.textContent = `${shown.length} of ${points.length} groups${additive ? ` · total ${number(total)} ${panel.unit}` : ' · individual resource ratios'}`;
      if (!shown.length) { visual.append(node('p', 'No groups match this filter.', 'empty')); return; }
      if (asTable) { visual.append(table(['Group', panel.unit || 'Value', ...(additive ? ['Share of total'] : [])], shown.map(p => [p.label, number(p.value), ...(additive ? [`${total ? number(p.value / total * 100) : '0'}%`] : [])]))); return; }
      if (panel.kind === 'donut') {
        const layout = node('div', undefined, 'donut-layout');
        const circle = node('div', undefined, 'donut'); circle.setAttribute('role', 'img'); circle.setAttribute('aria-label', `${panel.title}: ${number(total)} ${panel.unit}. Values follow in the legend.`);
        let offset = 0;
        const stops = points.map((p, i) => { const next = offset + (total ? p.value / total * 100 : 0); const stop = `${palette[i % palette.length]} ${offset}% ${next}%`; offset = next; return stop; });
        if (total) circle.style.background = `conic-gradient(${stops.join(',')})`;
        const center = node('div', undefined, 'donut-center'); center.append(node('strong', number(total)), node('small', panel.unit)); circle.append(center);
        const legend = node('div', undefined, 'legend');
        for (const point of shown) {
          const button = node('button'); button.setAttribute('aria-pressed', 'false');
          const dot = node('span', undefined, 'swatch'); dot.style.background = palette[points.indexOf(point) % palette.length];
          button.append(dot, node('span', point.label), node('strong', number(point.value)));
          button.addEventListener('click', () => select(point, button)); legend.append(button);
        }
        layout.append(circle, legend); visual.append(layout);
      } else {
        const list = node('div', undefined, 'bar-list');
        const max = Math.max(additive ? 1 : 100, ...points.map(p => p.value));
        for (const point of shown) {
          const button = node('button', undefined, 'chart-bar'); button.setAttribute('aria-pressed', 'false');
          const caption = node('span', undefined, 'bar-caption'); caption.append(node('span', point.label), node('strong', `${number(point.value)} ${panel.unit}`));
          const track = node('div', undefined, 'bar-track'); const fill = node('div', undefined, 'bar-fill'); fill.style.width = `${point.value / max * 100}%`; track.append(fill);
          button.append(caption, track); button.addEventListener('click', () => select(point, button)); list.append(button);
        }
        visual.append(list);
      }
    }
    search.addEventListener('input', draw);
    toggle.addEventListener('click', () => { asTable = !asTable; toggle.textContent = asTable ? 'Chart' : 'Data table'; toggle.setAttribute('aria-pressed', String(asTable)); draw(); });
    container.append(controls, visual, detail); draw();
  }
  function register(id, render) {
    if (renderers.has(id) || typeof render !== 'function') throw new Error(`Invalid renderer registration: ${id}`);
    renderers.set(id, render);
  }
  function render(container, panel) {
    const id = panel.kind === 'custom' ? panel.renderer : panel.kind;
    const renderer = renderers.get(id);
    if (!renderer) { container.append(node('p', 'This view needs a renderer that is not installed. Bounded data is shown below.', 'empty'), node('pre', JSON.stringify(panel.data ?? panel, null, 2))); return; }
    try { renderer(container, panel, { node, number, table, palette }); }
    catch { container.replaceChildren(node('p', 'This panel could not be displayed. The saved snapshot is unchanged.', 'empty')); }
  }
  register('bars', charts); register('donut', charts);
  register('table', (container, panel) => container.append(table(panel.columns, panel.rows)));
  window.apexViews = Object.freeze({ register, render, node, number, table });
})();
