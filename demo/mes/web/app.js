import {icon} from './icons.js';

const $ = (query, root = document) => root.querySelector(query);
const esc = value => String(value ?? '').replace(/[&<>"']/g, char => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]));
const number = value => Number(value).toLocaleString('en-GB');
const displayDate = value => value ? new Date(value).toLocaleString('en-GB', {day:'2-digit', month:'short', hour:'2-digit', minute:'2-digit'}) : '—';
const state = {page:'orders', offset:0, q:'', sort:'id', direction:'asc', filter:'', exact:null, rows:[], meta:null, references:{}, planningTab:'dispatch'};
let requestSequence = 0, toastTimer, previousFocus;

async function api(path, options = {}) {
  const response = await fetch(path, {headers:{'Content-Type':'application/json'}, ...options});
  const data = await response.json();
  if (!response.ok) throw new Error(data.error || 'The request failed');
  return data;
}
function notify(message) {
  clearTimeout(toastTimer); $('#toast').textContent = message; $('#toast').hidden = false;
  toastTimer = setTimeout(() => {$('#toast').hidden = true;}, 4000);
}
function badge(value) {
  const color = ['Complete','Present','Available','Received'].includes(value) ? 'green' : ['Running','In progress'].includes(value) ? 'blue' : ['Urgent','Absent','Unavailable','Delayed'].includes(value) ? 'red' : ['High','Maintenance','Training'].includes(value) ? 'amber' : '';
  return `<span class="badge ${color}">${esc(value)}</span>`;
}
function navItem(page, label, count = '') {
  return `<button class="nav-item ${state.page===page?'active':''}" data-nav="${page}" title="${label}">${icon(page)}<span>${label}</span>${count ? `<span class="count">${count}</span>` : ''}</button>`;
}
function shell() {
  const m = state.meta;
  $('#sidebar').innerHTML = `<div class="brand"><div class="brand-mark">N</div><div><strong>northstar</strong><small>MANUFACTURING</small></div></div>
    <div class="nav-section">SHOP FLOOR</div>${navItem('orders','Production',m.counts.orders)}${navItem('receipts','Inbound deliveries')}${navItem('downtime','Maintenance')}
    <div class="nav-section">MASTER DATA</div>${navItem('items','Articles & workplans')}${navItem('machines','Equipment')}${navItem('materials','Material stock')}${navItem('personnel','People & shifts')}
    <div class="nav-section">PLANNING</div>${navItem('workbook','Excel planning')}
    <div class="nav-bottom">${navItem('guide','Factory guide')}<button class="nav-item" id="reset-demo" title="Reset demo">${icon('reset')}<span>Reset demo</span></button><div class="environment"><span class="dot"></span> SYNTHETIC DEMO · V1.0</div></div>`;
  const label = state.meta.catalog[state.page]?.label || (state.page==='workbook'?'Excel planning':'Factory guide');
  const section = ['orders','lots','operations'].includes(state.page) ? 'Production' : label;
  $('#topbar').innerHTML = `<div class="breadcrumb">Plant 01 ${icon('chevron')} <strong>${esc(section)}</strong></div><div class="top-right"><span class="snapshot">${icon('clock')} Snapshot · 05 Oct 2026, 10:00 CEST</span><span class="dot"></span><span>Local MES</span><span class="avatar" title="Demo planner">PL</span></div>`;
  for (const button of document.querySelectorAll('[data-nav]')) button.addEventListener('click', () => navigate(button.dataset.nav));
  $('#reset-demo').addEventListener('click', resetDialog);
  // Keep parent navigation selected when a subordinate table is open.
  const parent = {lots:'orders',operations:'orders',routings:'items',shifts:'personnel'}[state.page];
  if (parent) $(`[data-nav="${parent}"]`).classList.add('active');
}
async function reloadMeta() { state.meta = await api('/api/meta'); state.references = {}; }
async function navigate(page, exact = null) {
  closeDrawer(); Object.assign(state, {page, offset:0, q:'', sort:'id', direction:'asc', filter:'', exact});
  history.replaceState(null, '', `#${page}`); await render(); window.scrollTo(0,0);
}
function stats() {
  const counts = state.meta.counts;
  return `<div class="stats">${[
    ['Customer orders',counts.orders,'Delivery commitments'],['Production lots',counts.lots,'Fixed lots · make to order'],
    ['Work operations',counts.operations,'Machining through final test'],['People on roster',counts.personnel,'Two shifts · shared qualifications'],
  ].map(([label,value,detail]) => `<div class="stat"><div class="stat-label">${label}</div><div class="stat-value">${number(value)}</div><div class="stat-detail">${detail}</div></div>`).join('')}</div>`;
}
function tabs(entries) {
  return `<div class="tabs">${entries.map(([key,label]) => `<button class="tab ${state.page===key?'active':''}" data-tab="${key}">${label}<small>${state.meta.counts[key]}</small></button>`).join('')}</div>`;
}
async function render() {
  shell();
  if (state.page === 'guide') return renderGuide();
  if (state.page === 'workbook') return renderWorkbook();
  const schema = state.meta.catalog[state.page];
  const production = ['orders','lots','operations'].includes(state.page);
  const title = production ? 'Production' : ['items','routings'].includes(state.page) ? 'Articles & workplans' : ['personnel','shifts'].includes(state.page) ? 'People & shifts' : schema.label;
  const subtitle = production ? 'One view of every order, lot and operation on the shop floor.' : schema.description + '.';
  let tabBar = production ? tabs([['orders','Customer orders'],['lots','Production lots'],['operations','Operations']]) : ['items','routings'].includes(state.page) ? tabs([['items','Articles'],['routings','Workplans']]) : ['personnel','shifts'].includes(state.page) ? tabs([['personnel','Personnel'],['shifts','Shift calendars']]) : '';
  const filterField = schema.columns.find(col => col.key === 'status' || col.key === 'attendance');
  const choices = state.exact ? [] : filterField?.choices || (state.page==='orders'||state.page==='lots' ? ['Released','In progress','Complete'] : state.page==='operations' ? ['Waiting','Running','Complete'] : []);
  $('#main').innerHTML = `<div class="page-title"><div><div class="eyebrow">${production?'OPERATIONS / OVERVIEW':'PLANT 01 / MASTER DATA'}</div><h1>${title}</h1><p class="subtitle">${subtitle}</p></div>${schema.create?`<button id="add-record" class="button primary">${icon('plus')} ${state.page==='orders'?'New order':'Add record'}</button>`:''}</div>
    ${production?stats():''}${tabBar}
    ${state.exact?`<div class="notice">${icon('info')} Showing ${esc(state.exact.value)}. <button id="clear-filter" class="button text">Show all</button></div>`:''}
    <section class="panel"><div class="toolbar"><div class="toolbar-left"><label class="search">${icon('search')}<input id="search" aria-label="Search records" placeholder="Search ${schema.label.toLowerCase()}…" value="${esc(state.q)}"></label>${choices.length?`<select id="status-filter" aria-label="Filter status"><option value="">All statuses</option>${choices.map(v=>`<option>${v}</option>`).join('')}</select>`:''}</div><div class="toolbar-right"><button id="export" class="button">${icon('download')} Export CSV</button></div></div><div class="table-scroll" id="table"><div class="loading">Loading records…</div></div><div class="pagination" id="pagination"></div></section>
    <div class="table-footnote">${icon('info')} ${state.page==='operations'?'Open an operation to record production progress.':'Select a row to view details or edit a record.'} Changes are saved in the local MES.</div>`;
  for (const tab of document.querySelectorAll('[data-tab]')) tab.addEventListener('click', () => navigate(tab.dataset.tab));
  $('#add-record')?.addEventListener('click', () => editDialog());
  $('#clear-filter')?.addEventListener('click', () => navigate(state.page));
  let timer;
  $('#search').addEventListener('input', event => {state.q=event.target.value;state.offset=0;clearTimeout(timer);timer=setTimeout(loadRows,180);});
  $('#status-filter')?.addEventListener('change', event => {state.filter=event.target.value;state.offset=0;loadRows();});
  if ($('#status-filter')) $('#status-filter').value=state.filter;
  $('#export').addEventListener('click', () => {location.href=`/api/tables/${state.page}?${queryString()}&format=csv`;});
  await loadRows();
}
function queryString() {
  const params = new URLSearchParams({q:state.q, offset:state.offset, limit:25, sort:state.sort, direction:state.direction});
  if (state.exact) {params.set('filter_field',state.exact.field);params.set('filter_value',state.exact.value);}
  else if (state.filter) {params.set('filter_field',state.page==='personnel'?'attendance':'status');params.set('filter_value',state.filter);}
  return params;
}
const tableColumns = {
  orders:['id','customer','item_id','quantity','due','priority','status'],
  lots:['id','order_id','item_id','quantity','status','location'],
  operations:['id','lot_id','name','group','setup_minutes','run_minutes','status'],
  items:['id','name','variant','size','material','lot_size'],
  machines:['id','name','group','capability','calendar','status'],
  personnel:['id','name','team','shift_id','attendance'],
  materials:['id','name','stock','unit','reorder_point','location'],
};
function cell(row, col) {
  const value = row[col.key];
  if (['status','attendance'].includes(col.key)) return badge(value);
  if (col.key==='priority') return `<span class="priority ${esc(value)}">${esc(value)}</span>`;
  if (col.type==='datetime') return displayDate(value);
  if (col.type==='integer'||col.type==='number') return number(value);
  if (col.key==='item_id') {
    const item = state.references.items?.find(item=>item.id===value);
    return `<strong>${esc(value)}</strong>${item?`<span class="sub">${esc(item.name)}</span>`:''}`;
  }
  return esc(value || '—');
}
async function loadRows() {
  const request = ++requestSequence;
  try {
    const [data] = await Promise.all([api(`/api/tables/${state.page}?${queryString()}`), reference('items')]);
    if (request !== requestSequence || !$('#table')) return;
    state.rows = data.rows;
    const schema = state.meta.catalog[state.page];
    const cols = tableColumns[state.page] || schema.columns.filter(col=>col.key!=='note');
    const columns = cols.map(col=>typeof col==='string'?schema.columns.find(c=>c.key===col):col);
    $('#table').innerHTML = `<table><thead><tr>${columns.map(col=>`<th aria-sort="${state.sort===col.key?(state.direction==='asc'?'ascending':'descending'):'none'}"><button data-sort="${col.key}">${esc(col.label)} ${state.sort===col.key?(state.direction==='asc'?'↑':'↓'):''}</button></th>`).join('')}<th></th></tr></thead><tbody>${data.rows.map((row,index)=>`<tr class="clickable" tabindex="0" data-row="${index}" aria-label="Open ${esc(row.id)}">${columns.map(col=>`<td class="${col.key==='id'?'id':''}">${cell(row,col)}</td>`).join('')}<td class="row-arrow">${icon('chevron')}</td></tr>`).join('')}</tbody></table>${!data.rows.length?'<div class="empty">No matching records. Try another search or filter.</div>':''}`;
    $('#pagination').innerHTML = `<span>${data.total ? `${data.offset+1}–${Math.min(data.offset+data.limit,data.total)}`:'0'} of ${number(data.total)} records</span><div class="pages"><button id="previous" aria-label="Previous page" ${!data.offset?'disabled':''}>‹</button><span>Page ${Math.floor(data.offset/data.limit)+1}</span><button id="next" aria-label="Next page" ${data.offset+data.limit>=data.total?'disabled':''}>›</button></div>`;
    for (const row of document.querySelectorAll('[data-row]')) {
      const open = () => editDialog(state.rows[Number(row.dataset.row)]);
      row.addEventListener('click',open);row.addEventListener('keydown',event=>{if(event.key==='Enter')open();});
    }
    for (const button of document.querySelectorAll('[data-sort]')) button.addEventListener('click',()=>{state.direction=state.sort===button.dataset.sort&&state.direction==='asc'?'desc':'asc';state.sort=button.dataset.sort;state.offset=0;loadRows();});
    $('#previous').addEventListener('click',()=>{state.offset-=25;loadRows();});
    $('#next').addEventListener('click',()=>{state.offset+=25;loadRows();});
  } catch(error) { if($('#table')) $('#table').innerHTML=`<div class="connection-error">${esc(error.message)}</div>`; }
}
async function reference(entity) {
  if (!state.references[entity]) state.references[entity] = (await api(`/api/tables/${entity}?limit=200`)).rows;
  return state.references[entity];
}
function openDrawer(title, body, footer) {
  previousFocus = document.activeElement;
  $('#drawer').innerHTML=`<div class="drawer-head"><div><div class="eyebrow">NORTHSTAR / MES</div><h2 id="drawer-title">${title}</h2></div><button class="close" id="close-drawer" aria-label="Close dialog">${icon('close')}</button></div><div class="drawer-body">${body}<div id="form-error" class="form-error" hidden></div></div><div class="drawer-footer">${footer}</div>`;
  $('#drawer').hidden=false;$('#overlay').hidden=false;document.body.style.overflow='hidden';
  $('#close-drawer').addEventListener('click',closeDrawer);$('#cancel')?.addEventListener('click',closeDrawer);
  ($('#drawer input:not(:disabled)')||$('#close-drawer')).focus();
}
function closeDrawer() {$('#drawer').hidden=true;$('#overlay').hidden=true;document.body.style.overflow='';previousFocus?.focus();}
$('#overlay').addEventListener('click',closeDrawer);
document.addEventListener('keydown',event=>{
  if($('#drawer').hidden)return;
  if(event.key==='Escape')closeDrawer();
  if(event.key==='Tab'){
    const elements=[...$('#drawer').querySelectorAll('button,input,select,textarea,a')].filter(e=>!e.disabled);
    const first=elements[0],last=elements.at(-1);
    if(event.shiftKey&&document.activeElement===first){event.preventDefault();last.focus();}
    if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first.focus();}
  }
});
async function editDialog(row = null) {
  const entity=state.page, isNew=!row, schema=state.meta.catalog[entity];
  let columns=schema.columns;
  if(isNew&&entity==='orders') columns=[...columns.filter(c=>c.key!=='status'),{key:'lot_size',label:'Pieces per lot',type:'integer',required:true,editable:true}];
  await Promise.all(columns.filter(c=>c.ref).map(c=>reference(c.ref)));
  const input = col => {
    const disabled=!isNew&&!col.editable;
    let value=row?.[col.key] ?? (col.key==='lot_size'?20:col.key==='quantity'?100:col.key==='priority'?'Normal':'');
    const options=col.ref?state.references[col.ref].map(r=>[r.id,`${r.id} · ${r.name||r.customer||r.id}`]):col.choices?.map(v=>[v,v]);
    let control;
    if(options&&!disabled) control=`<select name="${col.key}" ${col.required?'required':''}>${value?'':'<option value="">Choose…</option>'}${options.map(([id,label])=>`<option value="${esc(id)}" ${id===value?'selected':''}>${esc(label)}</option>`).join('')}</select>`;
    else if(col.key==='note') control=`<textarea name="${col.key}">${esc(value)}</textarea>`;
    else control=`<input name="${col.key}" value="${esc(value)}" ${disabled?'disabled':''} ${col.required?'required':''} type="${col.type==='datetime'?'datetime-local':col.type==='time'?'time':['integer','number'].includes(col.type)?'number':'text'}" ${['integer','number'].includes(col.type)?`min="0" step="${col.type==='integer'?'1':'any'}"`:''}>`;
    return `<label class="field ${['name','steps','note'].includes(col.key)?'wide':''}"><span>${esc(col.label)}</span>${control}</label>`;
  };
  const editable=columns.some(c=>c.editable);
  openDrawer(isNew?(entity==='orders'?'New customer order':`Add ${schema.label.toLowerCase()}`):esc(row.id),
    `<form id="record-form"><div class="form-grid">${columns.map(input).join('')}</div></form>${entity==='orders'?`<div class="detail-callout">${isNew?'Saving creates the production lots and their complete work instructions. The existing Excel plan stays unchanged.':'Delivery and priority changes update the MES. Review the separate Excel plan after making changes.'}</div>`:''}${row&&entity==='orders'?'<div class="detail-actions"><button class="button" id="related-lots">View production lots '+icon('arrow')+'</button></div>':''}${row&&entity==='lots'?'<div id="operation-detail" class="detail-list"></div>':''}${row&&entity==='operations'?'<div class="detail-actions"><button class="button primary" id="book-progress">Record progress</button></div>':''}`,
    `<button class="button" id="cancel">Close</button>${editable||isNew?'<button class="button primary" id="save-record" type="submit" form="record-form">Save changes</button>':''}`);
  $('#related-lots')?.addEventListener('click',()=>navigate('lots',{field:'order_id',value:row.id}));
  $('#book-progress')?.addEventListener('click',()=>progressDialog(row));
  if(row&&entity==='lots') {
    const data=await api(`/api/tables/operations?filter_field=lot_id&filter_value=${encodeURIComponent(row.id)}&sort=sequence`);
    if($('#operation-detail')) $('#operation-detail').innerHTML=`<h3>Material flow</h3><table><tbody>${data.rows.map(op=>`<tr><td>${op.sequence}</td><td>${esc(op.name)}</td><td>${badge(op.status)}</td></tr>`).join('')}</tbody></table><button id="lot-operations" class="button text">Open work instructions ${icon('arrow')}</button>`;
    $('#lot-operations')?.addEventListener('click',()=>navigate('operations',{field:'lot_id',value:row.id}));
  }
  $('#record-form').addEventListener('submit',async event=>{
    event.preventDefault();const button=$('#save-record');button.disabled=true;
    try{
      const data=Object.fromEntries(new FormData(event.target));
      for(const col of columns) if(col.key in data&&['number','integer'].includes(col.type)) data[col.key]=Number(data[col.key]);
      await api(`/api/tables/${entity}${row?'/'+encodeURIComponent(row.id):''}`,{method:row?'PATCH':'POST',body:JSON.stringify({data,expected_version:row?._version})});
      closeDrawer();await reloadMeta();await render();notify(isNew?'Record created. Review the Excel plan.':'Changes saved to MES.');
    }catch(error){$('#form-error').textContent=error.message;$('#form-error').hidden=false;button.disabled=false;}
  });
}
async function progressDialog(row) {
  const machines=(await reference('machines')).filter(m=>m.group===row.group);
  openDrawer(`Record progress · ${esc(row.id)}`,`<form id="progress-form"><p class="subtitle">${esc(row.name)}. Book cumulative good quantity. Previous operations must be complete.</p><div class="form-grid detail-list"><label class="field"><span>Good quantity</span><input name="completed_quantity" type="number" min="${row.completed_quantity}" step="1" required value="${row.completed_quantity}"></label><label class="field"><span>Actual resource</span><select name="resource_id">${machines.map(m=>`<option ${m.id===row.resource_id?'selected':''}>${m.id}</option>`).join('')}</select></label></div></form>`, '<button class="button" id="cancel">Cancel</button><button class="button primary" type="submit" form="progress-form">Book progress</button>');
  $('#progress-form').addEventListener('submit',async event=>{
    event.preventDefault();const data=Object.fromEntries(new FormData(event.target));data.completed_quantity=Number(data.completed_quantity);data.expected_version=row._version;
    try{await api(`/api/progress/${row.id}/report`,{method:'POST',body:JSON.stringify(data)});closeDrawer();await reloadMeta();await render();notify('Production progress recorded.');}
    catch(error){$('#form-error').textContent=error.message;$('#form-error').hidden=false;}
  });
}
function resetDialog() {
  openDrawer('Restore the demo baseline', `<p class="subtitle">This replaces all local MES edits and the demo working copy of the Excel file with the original synthetic dataset. Downloaded Excel files are unaffected.</p><form id="reset-form"><label class="field detail-list"><span>Type RESET DEMO to continue</span><input name="confirmation" required autocomplete="off"></label></form>`, '<button class="button" id="cancel">Cancel</button><button class="button danger" type="submit" form="reset-form">Reset demo</button>');
  $('#reset-form').addEventListener('submit',async event=>{
    event.preventDefault();try{await api('/api/reset',{method:'POST',body:JSON.stringify(Object.fromEntries(new FormData(event.target)))});closeDrawer();await reloadMeta();await render();notify('Original factory baseline restored.');}
    catch(error){$('#form-error').textContent=error.message;$('#form-error').hidden=false;}
  });
}
async function renderWorkbook() {
  const [plan, skills] = await Promise.all([api('/api/plan'),api('/api/skills')]);
  if(state.page!=='workbook')return;
  const stale=plan.mes_revision>plan.source_revision;
  $('#main').innerHTML=`<div class="page-title"><div><div class="eyebrow">PLANNING / WORKBOOK</div><h1>The planner’s desk</h1><p class="subtitle">The working sequence, people assignments and qualifications live in Excel.</p></div></div>
    <div class="planning-hero"><div class="workbook-symbol">${icon('workbook')}</div><div><h2>Production planning · Weeks 41–42</h2><p>${number(plan.rows.length)} operations · 600 lots · 5 worksheets<br>Original planner baseline · 05 October 2026</p></div>${state.meta.workbook_available?`<a class="button primary" href="/downloads/production-planning.xlsx">${icon('download')} Download Excel</a>`:'<span class="badge amber">Workbook not installed</span>'}</div>
    ${stale?'<div class="notice">'+icon('info')+'The MES has changed since this workbook was prepared. Review the differences before releasing a revised plan.</div>':''}
    <div class="tabs"><button class="tab ${state.planningTab==='dispatch'?'active':''}" data-planning="dispatch">Dispatch plan<small>${number(plan.rows.length)}</small></button><button class="tab ${state.planningTab==='skills'?'active':''}" data-planning="skills">Qualification matrix<small>24</small></button></div>
    <div class="panel"><div class="toolbar"><div><h3>${state.planningTab==='dispatch'?'First operations in the baseline':'Who can do what'}</h3><p class="subtitle">Read-only preview of the original workbook data.</p></div><span class="badge">EXCEL OWNED</span></div><div class="table-scroll" id="workbook-table"></div></div>
    <div class="table-footnote">${icon('info')} Excel edits are not synchronized into this preview or the MES. The workbook is a separate planning source.</div>`;
  for(const tab of document.querySelectorAll('[data-planning]'))tab.addEventListener('click',()=>{state.planningTab=tab.dataset.planning;renderWorkbook();});
  if(state.planningTab==='dispatch') $('#workbook-table').innerHTML=`<table><thead><tr>${['Operation','Workplace','Sequence','Start','Finish','Person','Commitment'].map(h=>`<th>${h}</th>`).join('')}</tr></thead><tbody>${plan.rows.slice(0,35).map(row=>`<tr><td class="id">${esc(row.operation_id)}<span class="sub">${esc(row.operation)}</span></td><td>${row.machine_id}</td><td>${row.sequence}</td><td>${displayDate(row.start)}</td><td>${displayDate(row.end)}</td><td>${row.person_id}</td><td>${row.fixed==='Yes'?'<span class="badge amber">Fixed</span>':'<span class="badge">Proposed</span>'}</td></tr>`).join('')}</tbody></table>`;
  else $('#workbook-table').innerHTML=`<table><thead><tr>${['Person','Setup','Mechanical','Precision','Electrical','Calibration','Testing'].map(h=>`<th>${h}</th>`).join('')}</tr></thead><tbody>${skills.rows.map(row=>`<tr><td class="id">${esc(row.person_id)}<span class="sub">${esc(row.name)}</span></td>${['Setup','Mechanical','Precision','Electrical','Calibration','Testing'].map(key=>`<td>${row[key]?'<span class="skills-yes">'+icon('check')+'</span>':'<span class="skills-no">—</span>'}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
}
function renderGuide() {
  $('#main').innerHTML=`<div class="page-title"><div><div class="eyebrow">THE DEMO / FACTORY GUIDE</div><h1>A familiar factory. A complex plan.</h1><p class="subtitle">Meet Northstar Valve Works, a deliberately fictional make-to-order manufacturer.</p></div><button class="button" id="guide-production">Open production ${icon('arrow')}</button></div>
    <div class="guide-intro"><div><h2>From metal blank to tested valve assembly.</h2><p>Northstar produces distributor, regulator and sensor assemblies for equipment makers. Orders share machining capacity, assembly benches and specialist people. The MES tracks what must be made and what has happened. The planner coordinates the next steps in Excel.</p></div><div class="factory-monogram">N</div></div>${stats()}
    <div class="flow"><div class="flow-card"><span>01 / BODY MANUFACTURING</span><h3>Machine, deburr, wash</h3><p>Alternative CNC cells, changing fixtures and materials. The body becomes available to assembly after cleaning.</p></div>${icon('arrow')}<div class="flow-card"><span>02 / ASSEMBLY & QUALITY</span><h3>Assemble, calibrate, test</h3><p>Variant-specific routes, shared specialists and a qualified final test before shipment.</p></div></div>
    <h2>Three products, shared capacity</h2><div class="guide-grid">
    <div class="guide-card"><span class="badge green">D / DISTRIBUTOR</span><p>A machined body with seals, plugs and connectors. The shortest route still competes for the same people and equipment.</p><div class="route">Body manufacturing → Assembly → Leak test</div></div>
    <div class="guide-card"><span class="badge amber">R / REGULATOR</span><p>Control valves are installed and adjusted. Precision assembly and qualified functional testing become additional constraints.</p><div class="route">Body manufacturing → Assembly → Adjustment → Function test</div></div>
    <div class="guide-card"><span class="badge blue">S / SENSOR</span><p>Electronics add a specialist route. The lot returns to an assembly bench after calibration for final completion.</p><div class="route">Body manufacturing → Preassembly → Sensor installation → Calibration → Final assembly → Function test</div></div></div>
    <div class="guide-note"><h3>The Monday morning handover</h3><p>The snapshot is fixed at 05 October 2026, 10:00, Europe/Berlin. Some work is already complete or running. A sensor delivery is confirmed for Wednesday, CNC-03 has a spindle inspection, and QA-03 has a calibration appointment. The first two hours of upcoming work are marked as fixed in Excel.</p><h3>Try a normal planning change</h3><p>Create an urgent order, move an inbound delivery, or add machine downtime. The MES retains the edit and flags the workbook baseline for review. Use Reset demo to return to the original situation.</p><h3>What the current baseline represents</h3><p>All names, quantities and times are synthetic. The Excel plan is a reproducible conventional baseline, not an APEX-optimized result. It uses fixed lots and setup allowances, whole-lot transfer, resource calendars, staffing and confirmed material supply. Sequence-dependent setup optimization and live Excel synchronization are future integration work.</p></div>`;
  $('#guide-production').addEventListener('click',()=>navigate('orders'));
}
async function start() {
  try {await reloadMeta();const page=location.hash.slice(1);if(page in state.meta.catalog||['guide','workbook'].includes(page))state.page=page;await render();}
  catch(error){$('#main').innerHTML=`<div class="connection-error">Could not open the MES: ${esc(error.message)}</div>`;}
}
start();
