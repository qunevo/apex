import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
const root=path.resolve(import.meta.dirname,'..'),origin=process.env.APEX_VIEWER||'http://127.0.0.1:8765';
async function tool(name,args={}){const r=await fetch(origin+'/api/tool',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({name,arguments:args})});const value=await r.json();assert.ok(r.ok,JSON.stringify(value));return value;}
const source=await tool('problem.import',{path:'examples/dispatch-campaign.json'});
const result=await tool('schedule.create',{scenario_id:source.scenario_id});
const u=new URL(result.viewer_url);u.searchParams.set('task','A-02');
const browser=await chromium.launch({headless:true,executablePath:process.env.APEX_BROWSER||'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe'});
const page=await browser.newPage({viewport:{width:1500,height:1000}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
async function done(){await page.waitForFunction(()=>!document.querySelector('#factory').disabled);assert.ok(!await page.locator('#status').evaluate(e=>e.classList.contains('error')),await page.locator('#status').textContent());}
try{
 await page.goto(u.href);await done();assert.equal(await page.locator('#fork').isVisible(),false);
 assert.ok(await page.locator('.lock-badge').count()>0);assert.match(await page.locator('.dispatch-explanation').textContent(),/six-productive-hours/);assert.match(await page.locator('#inspector').textContent(),/excluded/);
 await page.screenshot({path:path.join(root,'docs/reports/v0.5-dispatch-viewer.png'),fullPage:true});
 await page.getByRole('button',{name:'Commitments & rules',exact:true}).click();await done();assert.match(await page.locator('#model-content').textContent(),/21600 productive seconds/);assert.match(await page.locator('#model-content').textContent(),/finishing-cell/);assert.match(await page.locator('#model-content').textContent(),/availability/);
 await page.screenshot({path:path.join(root,'docs/reports/v0.5-dispatch-rules.png'),fullPage:true});
 await page.reload();await done();assert.match(await page.locator('#model-content').textContent(),/six-productive-hours/);
 await page.setViewportSize({width:800,height:900});assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);assert.deepEqual(errors,[]);
 fs.writeFileSync(path.join(root,'docs/reports/v0.5-dispatch-viewer.json'),JSON.stringify({passed:true,url:u.href,rules_url:page.url(),page_errors:errors,flows:['pinned task deep link','mandatory policy reasons','resource/start commitments','read-oriented navigation','policy units and exceptions','declarative constraints','reload','800px viewport']},null,2));
 console.log(JSON.stringify({passed:true,url:u.href}));
}finally{await browser.close();}
