import {t} from './i18n.js';

// Execution facts come from MES bookings, never from planned Excel assignments.
export function createProduction({$, esc, api, badge, displayDate, editDialog, navigate}) {
  async function all(entity, field, value) {
    const rows = [];
    let page;
    do {
      page = await api(`/api/tables/${entity}?${new URLSearchParams({filter_field:field, filter_value:value, limit:200, offset:rows.length, sort:entity==='operations'?'sequence':'id'})}`);
      rows.push(...page.rows);
    } while(rows.length < page.total);
    return rows;
  }
  const machine = op => op.resource_id ? esc(op.resource_id) : t('No execution booking');
  function operationRows(ops) {
    return ops.map((op,i)=>`<button type="button" class="period-row execution-row" data-operation="${i}"><span><strong>${op.sequence} · ${esc(t(op.name))}</strong><span>${esc(op.id)}</span><span>${machine(op)}${op.person_id?` · ${esc(op.person_id)}`:''}${op.actual_start?` · ${t('Started')} ${displayDate(op.actual_start)}`:''}${op.actual_end?` · ${t('Finished')} ${displayDate(op.actual_end)}`:''}</span></span>${badge(op.status)}<span aria-hidden="true">›</span></button>`).join('');
  }
  function wire(root, ops) {
    for (const button of root.querySelectorAll('[data-operation]')) button.addEventListener('click',()=>editDialog(ops[Number(button.dataset.operation)],'operations'));
  }
  async function enhance(entity, row) {
    if (!row || !['orders','lots','operations'].includes(entity)) return;
    if (entity === 'operations') {
      const summary = `<div class="execution-summary"><div class="equipment-status">${badge(row.status)}<strong>${machine(row)}</strong></div><dl><dt>${t('Actual operator')}</dt><dd>${esc(row.person_id || '—')}</dd><dt>${t('Actual start')}</dt><dd>${displayDate(row.actual_start)}</dd><dt>${t('Actual finish')}</dt><dd>${displayDate(row.actual_end)}</dd></dl><p>${t('Execution bookings describe actual work. Waiting operations have no actual machine assignment.')}</p><div class="detail-actions"><button type="button" class="button" id="execution-lot">${t('Open production lot')}</button><button type="button" class="button" id="execution-order">${t('Open customer order')}</button></div></div>`;
      $('.drawer-body').insertAdjacentHTML('afterbegin',summary);
      $('#execution-lot').addEventListener('click',async()=>editDialog((await all('lots','id',row.lot_id))[0],'lots'));
      $('#execution-order').addEventListener('click',async()=>editDialog((await all('orders','id',row.order_id))[0],'orders'));
      return;
    }
    const ops = await all('operations',entity==='orders'?'order_id':'lot_id',row.id);
    if (!$('#record-form')) return;
    if (entity === 'lots') {
      const root = $('#operation-detail');
      $('#record-form').before(root);
      const running = ops.find(op=>op.status==='Running');
      root.innerHTML=`<h3>${t('Material flow')}</h3><p class="subtitle">${running?t('Currently running on {machine}',{machine:esc(running.resource_id)})+'.':t('No operation is currently running for this lot.')} ${t('Select an operation for its work instructions and execution history.')}</p>${operationRows(ops)}<button type="button" id="lot-operations" class="button text">${t('View all operations')}</button>`;
      wire(root,ops);
      $('#lot-operations').addEventListener('click',()=>navigate('operations',{field:'lot_id',value:row.id}));
    } else {
      const running = ops.filter(op=>op.status==='Running');
      $('#record-form').insertAdjacentHTML('beforebegin',`<div id="order-execution" class="detail-list"><h3>${t('Running operations')} · ${running.length}</h3><p class="subtitle">${t('An order can be in progress while its lots wait between steps.')}</p>${running.length?operationRows(running):`<p>${t('No operation is currently running for this order.')}</p>`}<button type="button" class="button" id="order-operations">${t('View all operations')}</button></div>`);
      wire($('#order-execution'),running);
      $('#order-operations').addEventListener('click',()=>navigate('operations',{field:'order_id',value:row.id}));
    }
  }
  return {enhance};
}
