import {t, locale} from './i18n.js';
// Focused workplan and released-instruction views for the table-based MES.
export function createDetails(context) {
  const {$,esc,api,badge,displayDate,state,openDrawer,closeDrawer,editDialog,navigate,reloadMeta,render,notify}=context;
  const table=(headers,data)=>`<div class="table-scroll"><table><thead><tr>${headers.map(h=>`<th>${esc(t(h))}</th>`).join('')}</tr></thead><tbody>${data.map(row=>`<tr>${row.map(v=>`<td>${typeof v==='number'?v.toLocaleString(locale()):v}</td>`).join('')}</tr>`).join('')}</tbody></table></div>`;
  async function record(entity,id){return (await api(`/api/tables/${entity}?filter_field=id&filter_value=${encodeURIComponent(id)}`)).rows[0];}
  async function isLocked(entity,row){
    if(!row)return false;
    if(entity==='routings')return row.status==='Released';
    if(!['routing_steps','routing_modes','routing_materials'].includes(entity))return false;
    const step=entity==='routing_steps'?row:await record('routing_steps',row.step_id);
    return (await record('routings',step.routing_id)).status==='Released';
  }
  async function action(path,payload){
    try{await api(path,{method:'POST',body:JSON.stringify(payload)});closeDrawer();await reloadMeta();await render();notify('MES updated. Review the separate Excel plan.');}
    catch(error){$('#form-error').textContent=error.message;$('#form-error').hidden=false;}
  }
  function revisionDialog(row){
    openDrawer('Create draft workplan revision',`<p class="subtitle">${t("Copies all steps, machine alternatives and component requirements. Existing lots retain their released instructions.")}</p><form id="revision-form" class="form-grid detail-list"><label class="field"><span>${t("New workplan ID")}</span><input name="id" required maxlength="24" placeholder="${esc(row.id)}-B"></label><label class="field"><span>${t("Revision")}</span><input name="revision" required placeholder="B"></label><label class="field wide"><span>${t("Description")}</span><input name="name" required value="${esc(row.name)}"></label></form>`, `<button class="button" id="cancel">${t("Close")}</button><button class="button primary" type="submit" form="revision-form">${t("Create draft")}</button>`);
    $('#revision-form').addEventListener('submit',event=>{event.preventDefault();action(`/api/workplans/${row.id}/revise`,{...Object.fromEntries(new FormData(event.target)),expected_version:row._version});});
  }
  async function enhance(entity,row){
    if(!row)return;
    const form=$('#record-form');
    if(entity==='routings'){
      const steps=(await api(`/api/tables/routing_steps?filter_field=routing_id&filter_value=${encodeURIComponent(row.id)}&sort=sequence&limit=200`)).rows;
      form.insertAdjacentHTML('beforebegin',`<div class="detail-list"><div class="detail-actions">${row.status==='Draft'?`<button id="release-route" class="button primary">${t("Check & release")}</button>`:''}<button id="revise-route" class="button">${t("Create draft revision")}</button></div><h3>${t("Operation sequence")}</h3><p class="subtitle">${t("Open a step for instructions, alternative equipment and component requirements.")}</p>${steps.map((s,i)=>`<button class="period-row" data-step="${i}"><span><strong>${s.sequence} · ${esc(t(s.name))}</strong><span>${esc(t(s.group))} · ${esc(t(s.skill))}${s.active?'':' · '+t('Inactive')}</span></span>›</button>`).join('')}${row.status==='Draft'?`<button id="add-step" class="button">${t("Add step")}</button>`:''}</div>`);
      for(const button of document.querySelectorAll('[data-step]'))button.addEventListener('click',()=>editDialog(steps[Number(button.dataset.step)],'routing_steps'));
      $('#add-step')?.addEventListener('click',()=>editDialog(null,'routing_steps',{routing_id:row.id,active:true,sequence:Math.max(0,...steps.map(s=>s.sequence))+10}));
      $('#revise-route').addEventListener('click',()=>revisionDialog(row));
      $('#release-route')?.addEventListener('click',()=>action(`/api/workplans/${row.id}/release`,{expected_version:row._version}));
    }
    if(entity==='routing_steps'){
      form.insertAdjacentHTML('beforebegin',`<div class="detail-list"><p class="subtitle">${esc(row.routing_id)} ${t("· Step")} ${row.sequence}</p><div class="detail-actions"><button class="button" id="step-modes">${t("Machine alternatives")}</button><button class="button" id="step-materials">${t("Component requirements")}</button><button class="button text" id="parent-route">${t("Back to workplan")}</button></div></div>`);
      $('#step-modes').addEventListener('click',()=>navigate('routing_modes',{field:'step_id',value:row.id}));
      $('#step-materials').addEventListener('click',()=>navigate('routing_materials',{field:'step_id',value:row.id}));
      $('#parent-route').addEventListener('click',async()=>editDialog(await record('routings',row.routing_id),'routings'));
    }
    if(entity==='items'){
      form.insertAdjacentHTML('beforebegin',`<div class="detail-actions"><button id="article-routes" class="button">${t("Approved workplans")}</button></div>`);
      $('#article-routes').addEventListener('click',()=>navigate('item_routings',{field:'item_id',value:row.id}));
    }
    if(entity==='materials')form.insertAdjacentHTML('beforebegin',`<div class="detail-callout"><strong>${row.on_hand} ${esc(row.unit)} ${t("on hand")}</strong><br>${t("Opening stock + received deliveries − posted component issues. Confirmed inbound supply is not yet usable.")}</div>`);
    if(entity==='operations'){
      const lot=await record('lots',row.lot_id);
      const siblings=(await api(`/api/tables/operations?filter_field=lot_id&filter_value=${row.lot_id}&sort=sequence`)).rows;
      const previous=siblings.filter(o=>o.sequence<row.sequence).at(-1);
      const input=previous?previous.completed_quantity:lot.quantity;
      form.insertAdjacentHTML('beforebegin',`<div class="detail-list"><div class="equipment-status">${badge(row.status)}<span>${esc(lot.routing_id)} ${t("· revision")} ${esc(lot.routing_revision)}</span></div><p class="subtitle">${previous&&previous.status!=='Complete'?t('Waiting for the previous step.'):t('{count} usable input pieces',{count:input})} · ${lot.quantity} ${t("originally released.")}</p><h3>${t("Released machine alternatives")}</h3>${table(['Resource','Setup min','Min / piece','Attendance'],row.machine_options.map(m=>[esc(m.resource_id),m.setup_minutes,m.minutes_per_unit,esc(t(m.attendance))]))}<h3>${t("Components at this operation")}</h3>${row.material_requirements.length?table(['Component','Per input piece','Planned quantity'],row.material_requirements.map(m=>[esc(m.material_id),m.quantity_per_unit,m.quantity])):`<p class="subtitle">${t("No additional components; continue with the preceding work in progress.")}</p>`}<div class="detail-callout">${t("These instructions were copied at release. Later master-data changes do not alter this lot. Machine times are standards; Excel owns the actual planning assignment.")}</div><div class="detail-actions"><button id="op-confirmations" class="button">${t("Confirmation history")}</button><button id="op-issues" class="button">${t("Material issues")}</button></div></div>`);
      $('#op-confirmations').addEventListener('click',()=>navigate('confirmations',{field:'operation_id',value:row.id}));
      $('#op-issues').addEventListener('click',()=>navigate('material_issues',{field:'operation_id',value:row.id}));
    }
  }
  function clockDialog(){
    openDrawer('Advance demo clock',`<p class="subtitle">${t("Availability and usable receipts follow this clock. Advancing it does not execute the Excel plan. Production still needs explicit confirmations.")}</p><form id="clock-form" class="detail-list"><label class="field"><span>${t("New snapshot · local plant time")}</span><input name="as_of" type="datetime-local" required value="${state.meta.factory.as_of}" min="${state.meta.factory.as_of}"></label></form>`,`<button class="button" id="cancel">${t("Close")}</button><button class="button primary" type="submit" form="clock-form">${t("Advance clock")}</button>`);
    $('#clock-form').addEventListener('submit',event=>{event.preventDefault();action('/api/clock',{...Object.fromEntries(new FormData(event.target)),expected_as_of:state.meta.factory.as_of});});
  }
  return {enhance,isLocked,clockDialog};
}
