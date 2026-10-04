import {t, valueLabel} from './i18n.js';

export function createFilters({$, esc, state, api, openDrawer, closeDrawer, loadRows}) {
  const count = () => Object.keys(state.filters).length;
  function summary() {
    const root = $('#active-filters');
    if (!root) return;
    const schema = state.meta.catalog[state.page];
    root.innerHTML = Object.entries(state.filters).map(([key,rule])=>{
      const label = t(schema.columns.find(c=>c.key===key).label);
      const detail = [rule.contains, rule.values?.map(v=>valueLabel(state.page,key,v) || t('(Empty)')).join(', '),
        rule.min!==undefined?`${t('From')} ${rule.min}`:'', rule.max!==undefined?`${t('Until')} ${rule.max}`:''].filter(Boolean).join(' · ');
      return `<button class="filter-chip" data-remove-filter="${key}" title="${esc(t('Remove filter'))}">${esc(label)}: ${esc(detail)} <span aria-hidden="true">×</span></button>`;
    }).join('') + (count()?`<button class="button text" id="clear-columns">${t('Clear all filters')}</button>`:'');
    for(const button of root.querySelectorAll('[data-remove-filter]'))button.addEventListener('click',()=>{delete state.filters[button.dataset.removeFilter];state.offset=0;summary();loadRows();});
    $('#clear-columns')?.addEventListener('click',()=>{state.filters={};state.offset=0;summary();loadRows();});
    $('#open-filters').textContent=`${t('Filters')}${count()?` · ${count()}`:''}`;
  }
  async function open(column) {
    const page = state.page;
    const params = new URLSearchParams({facets:1,limit:1});
    if(state.exact){params.set('filter_field',state.exact.field);params.set('filter_value',state.exact.value);}
    const {facets} = await api(`/api/tables/${page}?${params}`);
    if(state.page!==page)return;
    const columns = state.meta.catalog[page].columns.filter(c=>c.type!=='json');
    const sections = columns.map(col=>{
      const rule=state.filters[col.key]||{}, range=['number','integer','datetime'].includes(col.type);
      const searchable=!col.choices&&!['status','location','group','current_operation'].includes(col.key)&&col.type!=='boolean';
      const choices=[...new Set([...(facets[col.key]||[]),...(rule.values||[])])];
      const opened = column===col.key || (!column && Boolean(state.filters[col.key]));
      return `<details class="filter-section" data-filter-field="${col.key}" ${opened?'open':''}><summary>${esc(t(col.label))}${state.filters[col.key]?' •':''}</summary>${range?`<div class="form-grid"><label class="field"><span>${t('From')}</span><input data-bound="min" type="${col.type==='datetime'?'date':'number'}" step="${col.type==='integer'?'1':'any'}" value="${esc(rule.min??'')}"></label><label class="field"><span>${t('Until')}</span><input data-bound="max" type="${col.type==='datetime'?'date':'number'}" step="${col.type==='integer'?'1':'any'}" value="${esc(rule.max??'')}"></label></div>${col.type==='datetime'?`<p class="subtitle">${t('Dates include the whole day. Use the same date in both fields for one day.')}</p>`:''}`:` ${searchable?`<label class="field"><span>${t('Contains text')}</span><input data-contains value="${esc(rule.contains||'')}" placeholder="${t('Any text')}"></label>`:''}<label class="field"><span>${t('Find values')}</span><input data-choice-search placeholder="${t('Search choices…')}"></label><div class="filter-selection-actions"><button type="button" class="button text" data-select-visible>${t('Select shown')}</button><button type="button" class="button text" data-select-none>${t('Clear selection')}</button></div><div class="filter-values">${choices.map(value=>`<label class="filter-value"><input type="checkbox" value="${esc(value)}" ${rule.values?.includes(value)?'checked':''}><span>${esc(valueLabel(page,col.key,value)||t('(Empty)'))}</span></label>`).join('')}</div>`}</details>`;
    }).join('');
    openDrawer(t('Filter records'),`<p class="subtitle">${t('Columns combine with AND. Checked values within a column combine with OR. No selection means all values.')}</p><form id="filters-form">${sections}</form>`,`<button class="button" id="cancel">${t('Cancel')}</button><button class="button" id="clear-filter-form">${t('Clear all filters')}</button><button class="button primary" type="submit" form="filters-form">${t('Apply filters')}</button>`);
    for(const section of document.querySelectorAll('.filter-section')) {
      section.querySelector('[data-choice-search]')?.addEventListener('input',event=>{
        const q=event.target.value.toLocaleLowerCase();
        for(const label of section.querySelectorAll('.filter-value'))label.hidden=!label.textContent.toLocaleLowerCase().includes(q);
      });
      section.querySelector('[data-select-visible]')?.addEventListener('click',()=>{for(const label of section.querySelectorAll('.filter-value:not([hidden])'))label.querySelector('input').checked=true;});
      section.querySelector('[data-select-none]')?.addEventListener('click',()=>{for(const input of section.querySelectorAll('input[type=checkbox]'))input.checked=false;});
    }
    $('#clear-filter-form').addEventListener('click',()=>{for(const input of $('#filters-form').querySelectorAll('input')){input.value=input.type==='checkbox'?input.value:'';input.checked=false;}for(const label of document.querySelectorAll('.filter-value'))label.hidden=false;});
    $('#filters-form').addEventListener('submit',event=>{
      event.preventDefault();const filters={};
      for(const section of document.querySelectorAll('.filter-section')){
        const col=columns.find(c=>c.key===section.dataset.filterField), rule={};
        const contains=section.querySelector('[data-contains]')?.value.trim();
        if(contains)rule.contains=contains;
        const values=[...section.querySelectorAll('input[type=checkbox]:checked')].map(i=>i.value);
        if(values.length&&values.length<section.querySelectorAll('input[type=checkbox]').length)rule.values=values;
        for(const input of section.querySelectorAll('[data-bound]'))if(input.value!=='')rule[input.dataset.bound]=col.type==='datetime'?input.value:Number(input.value);
        if(rule.min!==undefined&&rule.max!==undefined&&rule.min>rule.max){$('#form-error').textContent=t('Filter end must not be before its start');$('#form-error').hidden=false;return;}
        if(Object.keys(rule).length)filters[col.key]=rule;
      }
      state.filters=filters;state.offset=0;closeDrawer();summary();loadRows();
    });
    if(column)document.querySelector(`[data-filter-field="${column}"]`)?.scrollIntoView({block:'nearest'});
  }
  return {open,summary,count};
}
