// Optional demo renderer. No source access, credentials, state writes or scheduling.
window.apexViews.register('demo.import-flow', (container, panel, { node, number }) => {
  const data = panel.data;
  const flow = node('div', undefined, 'source-flow');
  function source(title, count, detail, primary = false) {
    const box = node('div', undefined, `source-box${primary ? ' primary' : ''}`);
    box.append(node('span', title, 'eyebrow'), node('strong', count == null ? '—' : number(count), 'source-number'), node('small', detail));
    return box;
  }
  flow.append(source('MES total', data.mes, 'open and closed operations'), node('span', '→', 'flow-arrow'),
    source('Open input', data.remaining, 'remaining operations only', true), node('span', '←', 'flow-arrow'),
    source('Excel rows', data.excel, 'all operation proposals'));
  const coverage = node('div', undefined, 'coverage');
  const progress = node('progress'); progress.max = Math.max(1, data.remaining); progress.value = data.matched; progress.setAttribute('aria-label', 'Excel coverage of remaining operations');
  coverage.append(progress, node('span', `${number(data.matched)} matched`), node('span', `${number(data.missing)} without an Excel row`), node('span', `${data.closed == null ? 'Unknown' : number(data.closed)} closed in MES`));
  const evidence = node('details', undefined, 'source-evidence'); evidence.append(node('summary', 'Inspect source fingerprints'));
  for (const s of data.sources ?? []) evidence.append(node('p', `${s.label}${s.revision ? ` · revision ${s.revision}` : ''} · SHA256 ${s.sha256}`));
  container.append(flow, coverage, evidence);
});
