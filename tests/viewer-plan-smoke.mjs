import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const root=path.resolve(import.meta.dirname,'..');
const origin=process.env.APEX_VIEWER||'http://127.0.0.1:8765';
async function tool(name,args={}) {
  const r=await fetch(new URL('/api/tool',origin),{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({name,arguments:args})});
  const v=await r.json();assert.ok(r.ok,JSON.stringify(v));return v;
}
const imported=await tool('demo.create',{profile:'production'});
const base=await tool('schedule.create',{scenario_id:imported.scenario_id});
const fork=await tool('scenario.fork',{scenario_id:imported.scenario_id});
await tool('scenario.freeze',{scenario_id:fork.scenario_id,expected_revision:1,schedule_id:base.schedule_id,until:1000,dimensions:['start','resource','order']});
const candidate=await tool('schedule.create',{scenario_id:fork.scenario_id});
const url=new URL(candidate.viewer_url);url.searchParams.set('baseline',base.schedule_id);
const executablePath=process.env.APEX_BROWSER||(process.platform==='win32'?'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe':undefined);
const browser=await chromium.launch({headless:true,...(executablePath?{executablePath}:{})});
const page=await browser.newPage({viewport:{width:1600,height:1100}});
const errors=[],calls=[];
page.on('pageerror',e=>errors.push(e.message));
page.on('request',r=>{if(r.url().endsWith('/api/tool'))calls.push(r.postDataJSON().name);});
async function done(){await page.waitForFunction(()=>!document.querySelector('#factory').disabled);assert.ok(!await page.locator('#status').evaluate(e=>e.classList.contains('error')),await page.locator('#status').textContent());}
try {
  await page.goto(url.href);await done();
  for(const id of ['fork','apply','repair','outage','freeze','train','search-settings'])assert.equal(await page.locator('#'+id).isVisible(),false,id);
  assert.equal(await page.locator('.top-nav button:visible').count(),3);
  assert.equal(await page.locator('.cards .card:visible').count(),4);
  assert.ok(await page.locator('.freeze-zone').count()>0);
  assert.ok(await page.locator('.lock-badge').count()>0);
  assert.equal(await page.locator('#comparison-panel').isVisible(),true);
  await page.locator('.bar.fixed-start').first().click();await done();
  assert.match(await page.locator('#inspector').textContent(),/Start fixed at/);
  await page.screenshot({path:path.join(root,'docs/reports/viewer-plan.png'),fullPage:true});
  await page.locator('#ask-agent').click();
  assert.match(await page.locator('#agent-context').inputValue(),new RegExp(candidate.schedule_id));
  assert.match(await page.locator('#agent-context').inputValue(),/Selected operation:/);
  await page.locator('#ask-agent').click();
  await page.locator('[data-view="jobs"]').click();await done();
  assert.match(await page.locator('#model-content').textContent(),/ORDER-1/);
  await page.locator('[data-view="rules"]').click();await done();
  assert.match(await page.locator('#model-content').textContent(),/Locks/);
  await page.locator('[data-view="schedule"]').click();await done();
  const saved=page.url();await page.reload();await done();assert.equal(page.url(),saved);
  await page.setViewportSize({width:800,height:900});
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
  const readTools=new Set(['schedule.page','scenario.get','scenario.compare','task.inspect','model.page']);
  assert.ok(calls.every(name=>readTools.has(name)),JSON.stringify(calls));
  await page.locator('#toggle-workbench').click();await done();
  assert.equal(await page.locator('#fork').isVisible(),true);
  await page.locator('#toggle-workbench').click();await done();
  assert.equal(await page.locator('#fork').isVisible(),false);
  assert.deepEqual(errors,[]);
  fs.writeFileSync(path.join(root,'docs/reports/viewer-plan-tests.json'),JSON.stringify({passed:true,url:saved,flows:['lean navigation','no scenario/search controls','freeze badges and operation details','baseline comparison','agent context handoff','jobs and rules','deep link reload','800px viewport','read-only plan interactions','explicit workbench toggle'],page_errors:errors},null,2));
  console.log('Lean plan viewer passed.');
} finally { await browser.close(); }
