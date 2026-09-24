import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const root=path.resolve(import.meta.dirname,'..');
const origin=process.env.APEX_VIEWER||'http://127.0.0.1:8765';
async function call(name,args={}) {
  const r=await fetch(`${origin}/api/tools/${name}`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(args)});
  const value=await r.json(); assert.ok(r.ok,JSON.stringify(value)); return value;
}
const imported=await call('problem.import',{path:'examples/chain-routing.json'});
const prepared=await call('material.prepare',{scenario_id:imported.scenario_id,expected_revision:1});
assert.equal(prepared.allocation_preview,true);
const scenario_id=prepared.scenario_id;
const baseline=await call('schedule.create',{scenario_id,options:{route_choices:{'component-workplan':'slow'}}});
const improved=await call('schedule.improve',{scenario_id,schedule_id:baseline.schedule_id,options:{iterations:48,budget_ms:0,trainer:{population_size:4}}});
assert.equal(improved.route_choices['component-workplan'],'fast');
assert.ok(improved.metrics.makespan<baseline.metrics.makespan);
const materials=await call('model.page',{schedule_id:improved.schedule_id,section:'materials',limit:1});
assert.equal(materials.items.length,1);assert.ok(materials.total>=3);
const detail=await call('task.inspect',{schedule_id:improved.schedule_id,task:'PRODUCE-FAST'});
assert.equal(detail.dispatch_urgency.due,75);assert.equal(detail.dispatch_urgency.priority,9);
assert.ok(detail.dependencies.some(d=>d.before==='PRODUCE-FAST'&&d.after==='ASSEMBLE'));
assert.equal((await call('schedule.validate',{schedule_id:improved.schedule_id})).valid,true);
const url=new URL(origin);url.search=new URLSearchParams({schedule:improved.schedule_id,scenario:scenario_id,baseline:baseline.schedule_id,task:'PRODUCE-FAST'});
const browser=await chromium.launch({headless:true,executablePath:process.env.APEX_BROWSER||'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe'});
const page=await browser.newPage({viewport:{width:1500,height:1050}});const errors=[];
page.on('pageerror',e=>errors.push(e.message));
try {
  await page.goto(url.href);
  await page.waitForFunction(()=>!document.querySelector('#demo').disabled);
  assert.match(await page.locator('#inspector').textContent(),/Dispatch urgency/);
  assert.match(await page.locator('#inspector').textContent(),/shared-technician|independent-technician/);
  assert.equal(await page.locator('#comparison-all-kpis').evaluate(e=>e.open),false);
  assert.ok(await page.locator('#comparison > table tr').count()<=6);
  await page.screenshot({path:path.join(root,'docs/reports/v0.6-chain-viewer.png'),fullPage:true});
  await page.locator('#metrics-details summary').click();
  await page.waitForSelector('#metrics-table td');
  assert.match(await page.locator('#metrics-table').textContent(),/order_on_time_delivery/);
  assert.match(await page.locator('#metrics-table').textContent(),/experimental/);
  await page.locator('#metrics-more').click();
  await page.waitForFunction(()=>document.querySelectorAll('#metrics-table table').length>1);
  assert.match(await page.locator('#metrics-table').textContent(),/stage:1:processing_time/);
  await page.screenshot({path:path.join(root,'docs/reports/v0.6-kpis.png'),fullPage:true});
  await page.setViewportSize({width:800,height:1000});
  assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth+2));
  assert.deepEqual(errors,[]);
  fs.writeFileSync(path.join(root,'docs/reports/v0.6-viewer-migration.json'),JSON.stringify({passed:true,url:url.href,baseline_metrics:baseline.metrics,improved_metrics:improved.metrics,baseline_ms:baseline.elapsed_ms,improve_ms:improved.elapsed_ms,material_rows:materials.total,page_errors:errors},null,2));
  console.log(JSON.stringify({passed:true,url:url.href}));
} finally {await browser.close();}
