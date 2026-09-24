import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const root=path.resolve(import.meta.dirname,'..');
const reportDir=process.env.APEX_TEST_OUTPUT||path.join(root,'docs/reports');
fs.mkdirSync(reportDir,{recursive:true});
const origin=process.env.APEX_VIEWER||'http://127.0.0.1:8765';
async function call(name,args={}){
  const response=await fetch(`${origin}/api/tools/${name}`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(args)});
  const value=await response.json();assert.ok(response.ok,JSON.stringify(value));return value;
}
const source=await call('demo.create',{tasks:32});
const scenario_id=source.scenario_id;
const baseline=await call('schedule.create',{scenario_id});
const improved=await call('schedule.improve',{scenario_id,schedule_id:baseline.schedule_id,options:{iterations:96,budget_ms:0,improve:{evolution_share:0.3},trainer:{population_size:8,workers:2},plus:{workers:2}}});
assert.equal(improved.search.algorithm,'trainer_plus_ga');
assert.equal(improved.evaluations,96);
assert.equal(improved.search.phases.at(-1).name,'direct_ga');
assert.equal(improved.search.operators.length,10);
assert.equal((await call('schedule.validate',{schedule_id:improved.schedule_id})).valid,true);
const url=new URL(origin);url.search=new URLSearchParams({scenario:scenario_id,schedule:improved.schedule_id,baseline:baseline.schedule_id,view:'search'});
const browser=await chromium.launch({headless:true,executablePath:process.env.APEX_BROWSER||'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe'});
const page=await browser.newPage({viewport:{width:1500,height:1000}});const errors=[];page.on('pageerror',e=>errors.push(e.message));
try{
  await page.goto(url.href);await page.waitForSelector('#ga-operators');
  assert.equal(await page.locator('#ga-operators').evaluate(e=>e.open),false);
  assert.equal(await page.locator('#improve').isVisible(),false);
  assert.match(await page.locator('#model-content').textContent(),/direct_ga/);
  await page.locator('#ga-operators summary').click();
  assert.equal(await page.locator('#ga-operators tr').count(),11);
  assert.match(await page.locator('#ga-operators').textContent(),/apex:job_crossover@2/);
  assert.match(await page.locator('#ga-operators').textContent(),/Skipped/);
  await page.screenshot({path:path.join(reportDir,'v0.6-ga-viewer.png'),fullPage:true});
  await page.setViewportSize({width:800,height:1000});
  assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+2));
  assert.deepEqual(errors,[]);
  const report={passed:true,url:url.href,baseline_score:baseline.score,score:improved.score,phases:improved.search.phases,operators:improved.search.operators,page_errors:errors};
  fs.writeFileSync(path.join(reportDir,'v0.6-viewer-evolution.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify({passed:true,url:url.href}));
} finally {await browser.close();}
