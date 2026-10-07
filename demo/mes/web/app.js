import {t, translateError, valueLabel, locale, language, setLanguage} from './i18n.js';
import {createFilters} from './filters.js';
import {createProduction} from './production.js';
import {icon} from './icons.js';
import {createDetails} from './details.js';
import {createShowcase, isShowcase} from './showcase.js';

const $ = (query, root = document) => root.querySelector(query);
const esc = value => String(value ?? '').replace(/[&<>"']/g, char => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]));
const number = value => Number(value).toLocaleString(locale());
const displayDate = (value, withYear = false) => value ? new Date(value).toLocaleString(locale(), {day:'2-digit', month:'short', ...(withYear?{year:'numeric'}:{}), hour:'2-digit', minute:'2-digit'}) : '—';
const state = {page:'orders', offset:0, q:'', sort:'id', direction:'asc', filters:{}, exact:null, rows:[], meta:null, references:{}, planningTab:'dispatch'};
let requestSequence = 0, toastTimer, previousFocus;
const details = createDetails({$, esc, api, badge, displayDate, state, openDrawer, closeDrawer, editDialog, navigate, reloadMeta, render, notify});
const filters = createFilters({$, esc, state, api, openDrawer, closeDrawer, loadRows});
const productionDetails = createProduction({$, esc, api, badge, displayDate, editDialog, navigate});
const showcase = createShowcase({navigate});
const masterPages = ['items','routings','item_routings','routing_steps','routing_modes','routing_materials'];
const peoplePages = ['personnel','shifts','absences'];
const productionPages = ['orders','lots','operations','confirmations'];

async function api(path, options = {}) {
  const response = await fetch(path, {headers:{'Content-Type':'application/json'}, ...options});
  const data = await response.json();
  if (!response.ok) throw new Error(translateError(data.error || 'The request failed'));
  return data;
}
function notify(message) {
  clearTimeout(toastTimer); $('#toast').textContent = t(message); $('#toast').hidden = false;
  toastTimer = setTimeout(() => {$('#toast').hidden = true;}, 4000);
}
function badge(value) {
  const color = ['Complete','Present','Available','Received'].includes(value) ? 'green' : ['Running','In progress'].includes(value) ? 'blue' : ['Urgent','Absent','Unavailable','Delayed'].includes(value) ? 'red' : ['High','Maintenance','Training'].includes(value) ? 'amber' : '';
  return `<span class="badge ${color}">${esc(t(value))}</span>`;
}
function navItem(page, label, count = '') {
  label=t(label);
  return `<button class="nav-item ${state.page===page || isShowcase(page) && isShowcase(state.page)?'active':''}" data-nav="${page}" title="${label}">${icon(isShowcase(page)?'showcase':page)}<span>${label}</span>${count ? `<span class="count">${count}</span>` : ''}</button>`;
}
function shell() {
  const m = state.meta;
  $('#sidebar').setAttribute('aria-label',t('Main navigation'));
  $('#sidebar').innerHTML = `<div class="brand"><img class="brand-mark" src="/assets/qunevo-logo.svg" alt="Qunevo" width="38" height="38"><div class="brand-identity"><strong aria-hidden="true">QUNEVO<span>.</span></strong><small>DEMO MES</small></div></div>
    <div class="nav-section">${t("INTRODUCTION")}</div>${navItem(isShowcase(state.page)?state.page:showcase.lastPage,'Showcase')}
    <div class="nav-section">${t("SHOP FLOOR")}</div>${navItem('orders','Production',m.counts.orders)}${navItem('receipts','Inbound deliveries')}${navItem('downtime','Unavailability')}
    <div class="nav-section">${t("MASTER DATA")}</div>${navItem('items','Articles & workplans')}${navItem('machines','Equipment')}${navItem('materials','Material stock')}${navItem('personnel','People & shifts')}
    <div class="nav-section">${t("PLANNING")}</div>${navItem('workbook','Excel planning')}
    <div class="nav-bottom"><button class="nav-item" id="reset-demo" title="${t("Reset demo")}">${icon('reset')}<span>${t("Reset demo")}</span></button><div class="environment"><span class="dot"></span> ${t("Synthetic demo")} <span class="environment-version">V1.0</span></div></div>`;
  const label = state.meta.catalog[state.page]?.label || (state.page==='workbook'?'Excel planning':'Showcase');
  const section = productionPages.includes(state.page) ? 'Production' : label;
  $('#topbar').innerHTML = `<div class="breadcrumb"><span class="app-name">Qunevo Demo MES</span>${icon('chevron')}<strong>${esc(t(section))}</strong></div><div class="top-right"><span class="demo-snapshot" title="${t("All demo dates are fixed. Today means this snapshot, not the computer's date.")}">${icon('clock')} <span>${t("Demo snapshot")} · ${displayDate(m.factory.as_of,true)}<small>${esc(m.factory.timezone)} · ${t("Fixed")}</small></span></span><label class="language-switch"><span class="sr-only">${t('Language')}</span><select id="language" aria-label="${t('Language')}"><option value="de" ${language()==='de'?'selected':''}>DE</option><option value="en" ${language()==='en'?'selected':''}>EN</option></select></label><span class="avatar" title="${t("Demo planner")}">PL</span></div>`;
  for (const button of document.querySelectorAll('[data-nav]')) button.addEventListener('click', () => navigate(button.dataset.nav));
  $('#reset-demo').addEventListener('click', resetDialog);
  $('#language').addEventListener('change',async event=>{setLanguage(event.target.value);await render();});
  // Keep parent navigation selected when a subordinate table is open.
  const parent = {lots:'orders',operations:'orders',routings:'items',item_routings:'items',routing_steps:'items',routing_modes:'items',routing_materials:'items',shifts:'personnel',absences:'personnel',confirmations:'orders',material_issues:'materials'}[state.page];
  if (parent) $(`[data-nav="${parent}"]`).classList.add('active');
}
async function reloadMeta() { state.meta = await api('/api/meta'); state.references = {}; }
async function navigate(page, exact = null) {
  closeDrawer(); Object.assign(state, {page, offset:0, q:'', sort:'id', direction:'asc', filters:{}, exact});
  history.replaceState(null, '', `#${page}`); await render(); window.scrollTo(0,0);
}
function stats() {
  const counts = state.meta.counts;
  return `<div class="stats">${[
    ['Customer orders',counts.orders,'Delivery commitments'],['Production lots',counts.lots,'Fixed lots · make to order'],
    ['Work operations',counts.operations,'Machining through final test'],['People on roster',counts.personnel,'Two shifts · shared qualifications'],
  ].map(([label,value,detail]) => `<div class="stat"><div class="stat-label">${t(label)}</div><div class="stat-value">${number(value)}</div><div class="stat-detail">${t(detail)}</div></div>`).join('')}</div>`;
}
function tabs(entries) {
  return `<div class="tabs">${entries.map(([key,label]) => `<button class="tab ${state.page===key?'active':''}" data-tab="${key}">${t(label)}<small>${state.meta.counts[key]}</small></button>`).join('')}</div>`;
}
async function render() {
  ++requestSequence;
  shell();
  if (isShowcase(state.page)) return showcase.render(state.page);
  showcase.leave();
  if (state.page === 'workbook') return renderWorkbook();
  const schema = state.meta.catalog[state.page];
  const production = productionPages.includes(state.page);
  const title = production ? 'Production' : masterPages.includes(state.page) ? 'Articles & workplans' : peoplePages.includes(state.page) ? 'People & shifts' : schema.label;
  const subtitle = production ? 'One view of every order, lot and operation on the shop floor.' : state.page==='machines' ? t('Availability at {time} · Local plant time. Open equipment to manage its unavailable periods.',{time:displayDate(state.meta.factory.as_of)}) : t(schema.description) + '.';
  let tabBar = production ? tabs([['orders','Customer orders'],['lots','Production lots'],['operations','Operations'],['confirmations','Confirmations']]) : masterPages.includes(state.page) ? tabs([['items','Articles'],['routings','Workplans'],['item_routings','Article workplans']]) : peoplePages.includes(state.page) ? tabs([['personnel','Personnel'],['shifts','Shift calendars'],['absences','Absences']]) : ['materials','material_issues'].includes(state.page) ? tabs([['materials','Stock'],['material_issues','Material issues']]) : '';
  $('#main').innerHTML = `<div class="page-title"><div><div class="eyebrow">${t(production?'OPERATIONS / OVERVIEW':'PLANT 01 / MASTER DATA')}</div><h1>${t(title)}</h1><p class="subtitle">${t(subtitle)}</p></div>${schema.create?`<button id="add-record" class="button primary">${icon('plus')} ${t(state.page==='orders'?'New order':'Add record')}</button>`:''}</div>
    ${production?stats():''}${tabBar}
    ${state.exact?`<div class="notice">${icon('info')} ${t("Showing")} ${esc(state.exact.value)}. <button id="clear-filter" class="button text">${t("Show all")}</button></div>`:''}
    <section class="panel"><div class="toolbar"><div class="toolbar-left"><label class="search">${icon('search')}<input id="search" aria-label="${t("Search records")}" placeholder="${esc(t('Search {table}…',{table:t(schema.label)}))}" value="${esc(state.q)}"></label><button id="open-filters" class="button">${t('Filters')}</button></div><div class="toolbar-right"><button id="export" class="button">${icon('download')} ${t("Export CSV")}</button></div></div><div id="active-filters" class="active-filters"></div><div class="table-scroll" id="table"><div class="loading">${t("Loading records…")}</div></div><div class="pagination" id="pagination"></div></section>
    <div class="table-footnote">${icon('info')} ${t(state.page==='operations'?'Open an operation to record production progress.':'Select a row to view details or edit a record.')} ${t("Changes are saved in the local MES.")}</div>`;
  for (const tab of document.querySelectorAll('[data-tab]')) tab.addEventListener('click', () => navigate(tab.dataset.tab));
  if(state.page==='machines'){
    $('.page-title').insertAdjacentHTML('beforeend',`<button id="schedule-unavailability" class="button primary">${icon('plus')} ${t("Schedule unavailability")}</button>`);
    $('#schedule-unavailability').addEventListener('click',()=>unavailabilityDialog());
  }
  $('#add-record')?.addEventListener('click', () => state.page==='downtime'?unavailabilityDialog():editDialog(null,state.page,state.exact?{[state.exact.field]:state.exact.value}:{}));
  if(state.exact?.field==='step_id') {
    const step=(await api(`/api/tables/routing_steps?filter_field=id&filter_value=${encodeURIComponent(state.exact.value)}`)).rows[0];
    if(step) {
      $('.notice').insertAdjacentHTML('beforeend',`<button id="back-step" class="button text">${t("Back to step")}</button>`);
      $('#back-step').addEventListener('click',()=>editDialog(step,'routing_steps'));
      if(await details.isLocked('routing_steps',step))$('#add-record')?.remove();
    }
  }
  $('#clear-filter')?.addEventListener('click', () => navigate(state.page));
  let timer;
  $('#search').addEventListener('input', event => {state.q=event.target.value;state.offset=0;clearTimeout(timer);timer=setTimeout(loadRows,180);});
  $('#open-filters').addEventListener('click',()=>filters.open().catch(error=>notify(error.message)));
  filters.summary();
  $('#export').addEventListener('click', () => {location.href=`/api/tables/${state.page}?${queryString()}&format=csv`;});
  await loadRows();
}
function queryString() {
  const params = new URLSearchParams({q:state.q, offset:state.offset, limit:25, sort:state.sort, direction:state.direction});
  if (state.exact) {params.set('filter_field',state.exact.field);params.set('filter_value',state.exact.value);}
  if (filters.count()) params.set('filters',JSON.stringify(state.filters));
  return params;
}
const tableColumns = {
  orders:['id','customer','item_id','quantity','due','priority','status'],
  lots:['id','order_id','item_id','quantity','good_quantity','scrap_quantity','status','location','current_operation','resource_id'],
  operations:['id','lot_id','name','group','status','resource_id','person_id','actual_start'],
  items:['id','name','variant','size','material','lot_size'],
  routings:['id','name','family','revision','status'],
  machines:['id','name','group','capability','calendar','status'],
  personnel:['id','name','team','shift_id','attendance'],
  materials:['id','name','stock','on_hand','unit','reorder_point','location'],
};
function cell(row, col) {
  const value = row[col.key];
  if(state.page==='machines'&&col.key==='status')return badge(value)+`<span class="sub">${esc(availabilitySummary(row))}</span>`;
  if (['status','attendance'].includes(col.key)) return badge(value);
  if(col.type==='boolean')return t(value?'Yes':'No');
  if (col.key==='priority') return `<span class="priority ${esc(value)}">${esc(t(value))}</span>`;
  if (col.type==='datetime') return displayDate(value);
  if (col.type==='integer'||col.type==='number') return number(value);
  if (col.key==='item_id') {
    const item = state.references.items?.find(item=>item.id===value);
    return `<strong>${esc(value)}</strong>${item?`<span class="sub">${esc(t(item.name))}</span>`:''}`;
  }
  return esc(valueLabel(state.page,col.key,value) || '—');
}
async function loadRows() {
  const request = ++requestSequence;
  try {
    const [data] = await Promise.all([api(`/api/tables/${state.page}?${queryString()}`), reference('items')]);
    if (request !== requestSequence || !$('#table')) return;
    state.rows = data.rows;
    const schema = state.meta.catalog[state.page];
    const cols = tableColumns[state.page] || schema.columns.filter(col=>col.key!=='note'&&col.type!=='json');
    const columns = cols.map(col=>typeof col==='string'?schema.columns.find(c=>c.key===col):col);
    $('#table').innerHTML = `<table><thead><tr>${columns.map(col=>`<th aria-sort="${state.sort===col.key?(state.direction==='asc'?'ascending':'descending'):'none'}"><button data-sort="${col.key}">${esc(t(col.label))} ${state.sort===col.key?(state.direction==='asc'?'↑':'↓'):''}</button><button class="column-filter ${state.filters[col.key]?'selected':''}" data-column-filter="${col.key}" aria-label="${esc(t('Filter {column}',{column:t(col.label)}))}" title="${esc(t('Filter {column}',{column:t(col.label)}))}">⌄</button></th>`).join('')}<th></th></tr></thead><tbody>${data.rows.map((row,index)=>`<tr class="clickable" tabindex="0" data-row="${index}" aria-label="${esc(t('Open {id}',{id:row.id}))}">${columns.map(col=>`<td class="${col.key==='id'?'id':''}">${cell(row,col)}</td>`).join('')}<td class="row-arrow">${icon('chevron')}</td></tr>`).join('')}</tbody></table>${!data.rows.length?`<div class="empty">${t("No matching records. Try another search or filter.")}</div>`:''}`;
    $('#pagination').innerHTML = `<span>${t('{range} of {count} records',{range:data.total?`${data.offset+1}–${Math.min(data.offset+data.limit,data.total)}`:'0',count:number(data.total)})}</span><div class="pages"><button id="previous" aria-label="${t("Previous page")}" ${!data.offset?'disabled':''}>‹</button><span>${t("Page")} ${Math.floor(data.offset/data.limit)+1}</span><button id="next" aria-label="${t("Next page")}" ${data.offset+data.limit>=data.total?'disabled':''}>›</button></div>`;
    for (const row of document.querySelectorAll('[data-row]')) {
      const open = () => editDialog(state.rows[Number(row.dataset.row)]);
      row.addEventListener('click',open);row.addEventListener('keydown',event=>{if(event.key==='Enter')open();});
    }
    for (const button of document.querySelectorAll('[data-sort]')) button.addEventListener('click',()=>{state.direction=state.sort===button.dataset.sort&&state.direction==='asc'?'desc':'asc';state.sort=button.dataset.sort;state.offset=0;loadRows();});
    for(const button of document.querySelectorAll('[data-column-filter]'))button.addEventListener('click',()=>filters.open(button.dataset.columnFilter).catch(error=>notify(error.message)));
    $('#previous').addEventListener('click',()=>{state.offset-=25;loadRows();});
    $('#next').addEventListener('click',()=>{state.offset+=25;loadRows();});
  } catch(error) { if($('#table')) $('#table').innerHTML=`<div class="connection-error">${esc(error.message)}</div>`; }
}
async function reference(entity) {
  if (!state.references[entity]) {
    const result=[]; let page;
    do { page=await api(`/api/tables/${entity}?limit=200&offset=${result.length}`); result.push(...page.rows); } while(result.length<page.total);
    state.references[entity]=result;
  }
  return state.references[entity];
}
function openDrawer(title, body, footer) {
  previousFocus = document.activeElement;
  $('#drawer').innerHTML=`<div class="drawer-head"><div><div class="eyebrow">QUNEVO DEMO MES</div><h2 id="drawer-title">${t(title)}</h2></div><button class="close" id="close-drawer" aria-label="${t("Close dialog")}">${icon('close')}</button></div><div class="drawer-body">${body}<div id="form-error" class="form-error" hidden></div></div><div class="drawer-footer">${footer}</div>`;
  $('#drawer').hidden=false;$('#overlay').hidden=false;document.body.style.overflow='hidden';
  $('#close-drawer').addEventListener('click',closeDrawer);$('#cancel')?.addEventListener('click',closeDrawer);
  ([...$('#drawer').querySelectorAll('input:not(:disabled),select:not(:disabled)')].find(element=>element.getClientRects().length)||$('#close-drawer')).focus();
}
function closeDrawer() {$('#drawer').hidden=true;$('#overlay').hidden=true;document.body.style.overflow='';previousFocus?.focus();}
$('#overlay').addEventListener('click',closeDrawer);
document.addEventListener('keydown',event=>{
  if($('#drawer').hidden)return;
  if(event.key==='Escape')closeDrawer();
  if(event.key==='Tab'){
    const elements=[...$('#drawer').querySelectorAll('button,input,select,textarea,a,summary')].filter(e=>!e.disabled&&e.getClientRects().length);
    const first=elements[0],last=elements.at(-1);
    if(event.shiftKey&&document.activeElement===first){event.preventDefault();last.focus();}
    if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first.focus();}
  }
});
function availabilitySummary(row) {
  if(row.permanently_unavailable)return t('Permanently unavailable');
  if(!row.unavailable_until)return t('No upcoming unavailable periods');
  return row.status==='Unavailable'?t('Until {time}',{time:displayDate(row.unavailable_until)}):t('Next: {start} – {end}',{start:displayDate(row.unavailable_from),end:displayDate(row.unavailable_until)});
}
function unavailabilityDialog(machineId='') {
  return editDialog(null,'downtime',{id:`DT-${Date.now()}`,resource_id:machineId,start:state.meta.factory.as_of});
}
async function equipmentPeriods(row) {
  const data=await api(`/api/tables/downtime?filter_field=resource_id&filter_value=${encodeURIComponent(row.id)}&sort=start&limit=200`);
  const section=$('#equipment-periods');if(!section)return;
  section.innerHTML=`<h3>${t("Unavailable periods")}</h3><p class="subtitle">${t("Start and end are required. Maintenance is a reason for a dated block.")}</p>${data.rows.map((period,i)=>`<button class="period-row" data-period="${i}"><span><strong>${displayDate(period.start)} → ${displayDate(period.end)}</strong><span>${esc(period.reason)}${period.cancelled?' · '+t('Cancelled'):''}</span></span>${icon('chevron')}</button>`).join('')||`<p class="subtitle">${t("No unavailable periods recorded.")}</p>`}${data.total>data.rows.length?`<p class="subtitle">${t("Showing the first 200 periods. Open Unavailability for the complete list.")}</p>`:''}<button class="button primary" id="add-period">${icon('plus')} ${t("Schedule unavailability")}</button>`;
  $('#add-period').addEventListener('click',()=>unavailabilityDialog(row.id));
  for(const button of section.querySelectorAll('[data-period]'))button.addEventListener('click',()=>editDialog(data.rows[Number(button.dataset.period)],'downtime'));
}
async function editDialog(row = null, entity=state.page, initial={}) {
  const isNew=!row, schema=state.meta.catalog[entity];
  let columns=schema.columns.filter(col=>!col.computed&&col.type!=='json'&&!(isNew&&col.key==='cancelled'));
  if(entity==='machines')columns=[...columns.filter(col=>col.key==='permanently_unavailable'),...columns.filter(col=>col.key!=='permanently_unavailable')];
  if(isNew&&entity==='orders') columns=[...columns.filter(c=>!['status','good_quantity','scrap_quantity'].includes(c.key)),{key:'lot_size',label:'Pieces per lot',type:'integer',required:true,editable:true}];
  const locked=await details.isLocked(entity,row);
  if(isNew&&entity==='routings')columns=columns.filter(c=>c.key!=='status');
  await Promise.all(columns.filter(c=>c.ref&&(isNew||c.editable)&&!locked).map(c=>reference(c.ref)));
  const input = col => {
    const disabled=locked||(!isNew&&!col.editable);
    let value=row?.[col.key] ?? initial[col.key] ?? (col.key==='active'?true:col.key==='quality_status'?'Released':col.key==='lot_size'?20:col.key==='quantity'?100:col.key==='priority'?'Normal':'');
    if(col.type==='boolean')return `<label class="field wide checkbox-field"><input type="checkbox" name="${col.key}" ${value?'checked':''} ${disabled?'disabled':''}><span>${esc(t(col.label))}${col.key==='permanently_unavailable'?`<small>${t("Use only when this equipment is out of service indefinitely. Dated periods remain in effect when unchecked.")}</small>`:''}</span></label>`;
    const options=col.ref&&!disabled?state.references[col.ref].map(r=>[r.id,`${r.id} · ${r.name?valueLabel(col.ref,'name',r.name):r.customer||r.id}`]):col.choices?.map(v=>[v,t(v)]);
    let control;
    if(options&&!disabled) control=`<select name="${col.key}" ${col.required?'required':''}>${value?'':`<option value="">${t("Choose…")}</option>`}${options.map(([id,label])=>`<option value="${esc(id)}" ${id===value?'selected':''}>${esc(label)}</option>`).join('')}</select>`;
    else if(['note','instruction','hold_reason'].includes(col.key)) control=`<textarea name="${col.key}" ${disabled?'disabled':''}>${esc(disabled?valueLabel(entity,col.key,value):value)}</textarea>`;
    else control=`<input name="${col.key}" value="${esc(disabled?valueLabel(entity,col.key,value):value)}" ${disabled?'disabled':''} ${col.required?'required':''} type="${col.type==='datetime'?'datetime-local':col.type==='time'?'time':['integer','number'].includes(col.type)?'number':'text'}" ${['integer','number'].includes(col.type)?`min="0" step="${col.type==='integer'?'1':'any'}"`:''}>`;
    return `<label class="field ${['name','steps','note','instruction','hold_reason','output'].includes(col.key)?'wide':''}"><span>${esc(t(col.label))}</span>${control}</label>`;
  };
  const editable=!locked&&columns.some(c=>c.editable);
  openDrawer(isNew?(entity==='orders'?'New customer order':t('Add {table}',{table:t(schema.label)})):esc(row.id),
    `<form id="record-form"><div class="form-grid">${columns.map(input).join('')}</div></form>${locked?`<div class="detail-callout">${t("Released revision · instructions are locked. Create a draft revision to make changes.")}</div>`:''}${entity==='orders'?`<div class="detail-callout">${t(isNew?'Saving creates the production lots and their complete work instructions. The existing Excel plan stays unchanged.':'Delivery and priority changes update the MES. Review the separate Excel plan after making changes.')}</div>`:''}${row&&entity==='orders'?`<div class="detail-actions"><button class="button" id="related-lots">${t('View production lots')} `+icon('arrow')+`</button></div>`:''}${row&&entity==='lots'?`<div id="operation-detail" class="detail-list"></div>`:''}${row&&entity==='operations'&&!['Complete','Skipped'].includes(row.status)?`<div class="detail-actions"><button class="button primary" id="book-progress">${t("Record progress")}</button></div>`:''}`,
    `<button class="button" id="cancel">${t("Close")}</button>${editable||isNew?`<button class="button primary" id="save-record" type="submit" form="record-form">${t("Save changes")}</button>`:''}`);
  $('#related-lots')?.addEventListener('click',()=>navigate('lots',{field:'order_id',value:row.id}));
  $('#book-progress')?.addEventListener('click',()=>progressDialog(row));
  if(row&&entity==='machines'){
    $('#record-form').insertAdjacentHTML('beforebegin',`<div class="equipment-status">${badge(row.status)}<span>${esc(availabilitySummary(row))}</span></div>`);
    $('#record-form').insertAdjacentHTML('beforebegin',`<div class="detail-list" id="equipment-periods"></div>`);
    await equipmentPeriods(row);
  }
  await details.enhance(entity,row);
  await productionDetails.enhance(entity,row);
  if(isNew&&entity==='orders') {
    await reference('item_routings');
    const form=$('#record-form');
    const selectWorkplans=()=>{
      const article=state.references.items.find(i=>i.id===form.elements.item_id.value);
      const allowed=state.references.item_routings.filter(r=>r.item_id===article?.id&&r.active);
      form.elements.routing_id.innerHTML=allowed.map(r=>`<option value="${esc(r.routing_id)}" ${r.routing_id===article?.routing_id?'selected':''}>${esc(r.routing_id)}</option>`).join('');
      if(article)form.elements.lot_size.value=article.lot_size;
    };
    form.elements.item_id.addEventListener('change',selectWorkplans);selectWorkplans();
  }
  $('#record-form').addEventListener('submit',async event=>{
    event.preventDefault();const button=$('#save-record');button.disabled=true;
    try{
      const data=Object.fromEntries(new FormData(event.target));
      for(const col of columns) if(col.key in data&&['number','integer'].includes(col.type)) data[col.key]=Number(data[col.key]);
      for(const col of columns) if(col.type==='boolean'&&col.editable)data[col.key]=event.target.elements.namedItem(col.key).checked;
      await api(`/api/tables/${entity}${row?'/'+encodeURIComponent(row.id):''}`,{method:row?'PATCH':'POST',body:JSON.stringify({data,expected_version:row?._version})});
      closeDrawer();await reloadMeta();await render();notify(isNew?'Record created. Review the Excel plan.':'Changes saved to MES.');
    }catch(error){$('#form-error').textContent=error.message;$('#form-error').hidden=false;button.disabled=false;}
  });
}
async function progressDialog(row) {
  const machines=(await reference('machines')).filter(m=>row.machine_options.some(o=>o.resource_id===m.id));
  const skills=(await api('/api/skills')).rows;
  const people=(await reference('personnel')).filter(p=>skills.some(s=>s.person_id===p.id&&s[row.skill]));
  openDrawer(t('Record progress · {id}',{id:esc(row.id)}),`<form id="progress-form"><p class="subtitle">${esc(t(row.name))}${t(". Book cumulative good and scrap quantities. Scrap reduces the input for the next step. Enter a finish when all input is accounted for. Times use the demo clock.")}</p><div class="form-grid detail-list"><label class="field"><span>${t("Good quantity")}</span><input name="completed_quantity" type="number" min="${row.completed_quantity}" step="1" required value="${row.completed_quantity}"></label><label class="field"><span>${t("Actual resource")}</span><select name="resource_id">${machines.map(m=>`<option ${m.id===row.resource_id?'selected':''}>${m.id}</option>`).join('')}</select></label><label class="field"><span>${t("Scrap quantity")}</span><input name="scrap_quantity" type="number" min="${row.scrap_quantity}" step="1" required value="${row.scrap_quantity}"></label><label class="field"><span>${t("Operator ·")} ${esc(t(row.skill))}</span><select name="person_id" required>${people.map(p=>`<option value="${p.id}" ${p.id===row.person_id?'selected':''}>${esc(p.name)} · ${p.id}</option>`).join('')}</select></label><label class="field"><span>${t("Actual start")}</span><input name="actual_start" type="datetime-local" required value="${row.actual_start||state.meta.factory.as_of}"></label><label class="field"><span>${t("Actual finish (when complete)")}</span><input name="actual_end" type="datetime-local" value="${row.actual_end}"></label><label class="field wide"><span>${t("Scrap reason")}</span><textarea name="scrap_reason">${esc(row.scrap_reason)}</textarea></label></div></form>`, `<button class="button" id="cancel">${t("Cancel")}</button><button class="button primary" type="submit" form="progress-form">${t("Book progress")}</button>`);
  $('#progress-form').addEventListener('submit',async event=>{
    event.preventDefault();const data=Object.fromEntries(new FormData(event.target));data.completed_quantity=Number(data.completed_quantity);data.scrap_quantity=Number(data.scrap_quantity);data.expected_version=row._version;
    try{await api(`/api/progress/${row.id}/report`,{method:'POST',body:JSON.stringify(data)});closeDrawer();await reloadMeta();await render();notify('Production progress recorded.');}
    catch(error){$('#form-error').textContent=error.message;$('#form-error').hidden=false;}
  });
}
function resetDialog() {
  openDrawer('Restore the demo baseline', `<p class="subtitle">${t("This replaces all local MES edits and the demo working copy of the Excel file with the original synthetic dataset. Downloaded Excel files are unaffected.")}</p><p class="subtitle">${t("Close the shared workbook in Excel before resetting. Reopen it afterward to use the restored file.")}</p><form id="reset-form"><label class="field detail-list"><span>${t("Type RESET DEMO to continue")}</span><input name="confirmation" required autocomplete="off"></label></form>`, `<button class="button" id="cancel">${t("Cancel")}</button><button class="button danger" type="submit" form="reset-form">${t("Reset demo")}</button>`);
  $('#reset-form').addEventListener('submit',async event=>{
    event.preventDefault();try{await api('/api/reset',{method:'POST',body:JSON.stringify(Object.fromEntries(new FormData(event.target)))});state.references={};closeDrawer();await reloadMeta();await render();notify('MES and shared Excel workbook restored. Reopen the workbook in Excel.');}
    catch(error){$('#form-error').textContent=error.message;$('#form-error').hidden=false;}
  });
}
async function renderWorkbook() {
  const [plan, skills, workbook] = await Promise.all([api('/api/plan'),api('/api/skills'),api('/api/workbook')]);
  if(state.page!=='workbook')return;
  const stale=plan.mes_revision>plan.source_revision;
  $('#main').innerHTML=`<div class="page-title"><div><div class="eyebrow">${t("PLANNING / WORKBOOK")}</div><h1>${t("The planner’s desk")}</h1><p class="subtitle">${t("The working sequence, people assignments and qualifications live in Excel.")}</p></div></div>
    <div class="planning-hero"><div class="workbook-symbol">${icon('workbook')}</div><div><h2>${t("Production planning · Weeks 41–42")}</h2><p>${number(plan.rows.length)} ${t("operations · 600 lots · 5 worksheets")}<br>${t("Original planner baseline · 05 October 2026")}</p></div>${workbook.available?`<button class="button primary" id="open-workbook">${icon('workbook')} ${t("Open in Excel")}</button>`:`<span class="badge amber">${t("Workbook not installed")}</span>`}</div>
    <div class="notice">${icon('info')} ${t("Open and save the shared working file. Both containers see your saved changes. MES data and the Excel plan are not synchronized automatically.")}</div>
    <p id="workbook-open-status" role="status" aria-live="polite">${workbook.opener_active?'':t("The desktop opener is unavailable. Run the demo starter again without --no-open.")}</p>
    ${workbook.host_path?`<p class="subtitle workbook-path">${t("Shared workbook:")} <code>${esc(workbook.host_path)}</code></p>`:''}
    ${stale?`<div class="notice">`+icon('info')+`The MES has changed since this workbook was prepared. Review the differences before releasing a revised plan.</div>`:''}
    <div class="tabs"><button class="tab ${state.planningTab==='dispatch'?'active':''}" data-planning="dispatch">${t("Dispatch plan")}<small>${number(plan.rows.length)}</small></button><button class="tab ${state.planningTab==='skills'?'active':''}" data-planning="skills">${t("Qualification matrix")}<small>24</small></button></div>
    <div class="panel"><div class="toolbar"><div><h3>${t(state.planningTab==='dispatch'?'First operations in the baseline':'Who can do what')}</h3><p class="subtitle">${t("Read-only preview of the original workbook data.")}</p></div><span class="badge">${t("EXCEL OWNED")}</span></div><div class="table-scroll" id="workbook-table"></div></div>
    <div class="table-footnote">${icon('info')} ${t("Excel edits are not synchronized into this preview or the MES. The workbook is a separate planning source.")}</div>`;
  $('#open-workbook')?.addEventListener('click', async event=>{
    const button=event.currentTarget, status=$('#workbook-open-status');
    button.disabled=true; status.textContent=t('Opening the shared workbook…');
    try {
      const opened=await api('/api/workbook/open',{method:'POST',body:'{}'});
      let outcome='pending';
      for(let attempt=0;attempt<12&&outcome==='pending';attempt++) {
        await new Promise(resolve=>setTimeout(resolve,500));
        outcome=(await api(`/api/workbook/open?request_id=${encodeURIComponent(opened.request_id)}`)).status;
      }
      status.textContent=t(outcome==='launched'?'The shared workbook was sent to your spreadsheet application.':outcome==='failed'?'Could not open the spreadsheet application. Open the shared workbook path manually.':outcome==='unavailable'?'The desktop opener is unavailable. Run the demo starter again without --no-open.':'Opening is taking longer than expected. Check your spreadsheet application before trying again.');
    } catch(error) { status.textContent=error.message; }
    finally { button.disabled=false; }
  });
  for(const tab of document.querySelectorAll('[data-planning]'))tab.addEventListener('click',()=>{state.planningTab=tab.dataset.planning;renderWorkbook();});
  if(state.planningTab==='dispatch') $('#workbook-table').innerHTML=`<table><thead><tr>${['Operation','Workplace','Sequence','Start','Finish','Person','Commitment'].map(h=>`<th>${t(h)}</th>`).join('')}</tr></thead><tbody>${plan.rows.slice(0,35).map(row=>`<tr><td class="id">${esc(row.operation_id)}<span class="sub">${esc(t(row.operation))}</span></td><td>${row.machine_id}</td><td>${row.sequence}</td><td>${displayDate(row.start)}</td><td>${displayDate(row.end)}</td><td>${row.person_id}</td><td>${row.fixed==='Yes'?`<span class="badge amber">${t("Fixed")}</span>`:`<span class="badge">${t("Proposed")}</span>`}</td></tr>`).join('')}</tbody></table>`;
  else $('#workbook-table').innerHTML=`<table><thead><tr>${['Person','Setup','Mechanical','Precision','Electrical','Calibration','Testing'].map(h=>`<th>${t(h)}</th>`).join('')}</tr></thead><tbody>${skills.rows.map(row=>`<tr><td class="id">${esc(row.person_id)}<span class="sub">${esc(t(row.name))}</span></td>${['Setup','Mechanical','Precision','Electrical','Calibration','Testing'].map(key=>`<td>${row[key]?`<span class="skills-yes">`+icon('check')+`</span>`:`<span class="skills-no">—</span>`}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
}
async function start() {
  try {await reloadMeta();const page=location.hash.slice(1);if(page in state.meta.catalog||page==='workbook'||isShowcase(page))state.page=page;await render();}
  catch(error){$('#main').innerHTML=`<div class="connection-error">${t("Could not open the MES:")} ${esc(error.message)}</div>`;}
}
window.addEventListener('hashchange', () => {
  const page = location.hash.slice(1);
  if (state.meta && (page in state.meta.catalog || page === 'workbook' || isShowcase(page))) navigate(page);
});
start();
