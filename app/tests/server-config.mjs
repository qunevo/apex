import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import net from 'node:net';
import {spawn} from 'node:child_process';
import {setTimeout as delay} from 'node:timers/promises';
import {chromium} from 'playwright';

const root=path.resolve(import.meta.dirname,'..');
const workspace=await fs.mkdtemp(path.join(os.tmpdir(),'apex configured '));
// A second test-only package exercises routing without shipping another domain.
const bundledConfig=path.join(root,'customization/demo/apex.config.json');
const config=JSON.parse(await fs.readFile(bundledConfig,'utf8'));
for(const id of config.enabled_customizations) {
  const folder=path.join(workspace,'customization',id);
  await fs.mkdir(folder,{recursive:true});
  await fs.copyFile(path.resolve(path.dirname(bundledConfig),config.customization_root,id,'package.json'),path.join(folder,'package.json'));
}
await fs.mkdir(path.join(workspace,'customization/secondary'));
await fs.writeFile(path.join(workspace,'customization/secondary/package.json'),JSON.stringify({id:'secondary',version:'1'}));
config.customization_root='customization';
config.enabled_customizations.push('secondary');
const configPath=path.join(workspace,'apex.config.json');
await fs.writeFile(configPath,JSON.stringify(config));
const reservation=net.createServer();
await new Promise(resolve=>reservation.listen(0,'127.0.0.1',resolve));
const port=reservation.address().port;
await new Promise(resolve=>reservation.close(resolve));
const origin=`http://127.0.0.1:${port}`;
const binary=process.env.APEX_BINARY||path.join(root,'target/release',process.platform==='win32'?'apex.exe':'apex');
const server=spawn(binary,['serve','--port',String(port),'--config',configPath,'--workspace',workspace],{
  cwd:workspace,windowsHide:true,env:{...process.env,APEX_BIND:'127.0.0.1',APEX_PUBLIC_URL:origin,APEX_API_TOKEN:''},stdio:['ignore','ignore','pipe']
});
let errors='';server.stderr.on('data',chunk=>errors+=chunk);
let browser;
async function tool(name,args={}) {
  const response=await fetch(`${origin}/api/tools/${name}`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(args)});
  const result=await response.json();assert.ok(response.ok,JSON.stringify(result));return result;
}
try {
  let ready=false;
  for(let i=0;i<100;i++) {
    assert.equal(server.exitCode,null,errors);
    try { if((await fetch(origin)).ok){ready=true;break;} } catch {}
    await delay(100);
  }
  assert.ok(ready,'Configured server did not start');
  const caps=await tool('capabilities');assert.equal(caps.customization_package.id,'demo');
  const source=await tool('demo.create',{customization:'secondary',tasks:4});
  const plan=await tool('schedule.create',{customization:'secondary',scenario_id:source.scenario_id});
  const rejected=await fetch(`${origin}/api/tools/scenario.get`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({scenario_id:source.scenario_id})});
  assert.equal(rejected.status,422);
  const rpc=await fetch(`${origin}/mcp`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',id:1,method:'tools/call',params:{name:'scenario.get',arguments:{customization:'secondary',scenario_id:source.scenario_id}}})});
  assert.equal((await rpc.json()).result.structuredContent.customization_package.id,'secondary');
  const executablePath=process.env.APEX_BROWSER||(process.platform==='win32'?'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe':undefined);
  browser=await chromium.launch({headless:true,...(executablePath?{executablePath}:{})});
  const page=await browser.newPage();const calls=[],pageErrors=[];
  page.on('pageerror',error=>pageErrors.push(error.message));
  page.on('request',request=>{if(request.url().endsWith('/api/tool'))calls.push(request.postDataJSON().arguments);});
  await page.goto(plan.viewer_url);
  await page.waitForFunction(()=>!document.querySelector('#factory').disabled);
  assert.equal(await page.locator('#status').evaluate(element=>element.classList.contains('error')),false);
  assert.ok(calls.length>0);assert.ok(calls.every(args=>args.customization==='secondary'));
  await page.locator('#ask-agent').click();
  assert.match(await page.locator('#agent-context').inputValue(),/Customization: secondary/);
  await page.locator('#toggle-workbench').click();
  await page.waitForFunction(()=>!document.querySelector('#factory').disabled);
  await page.locator('#demo').click();
  await page.waitForFunction(()=>!document.querySelector('#factory').disabled);
  assert.equal(new URL(page.url()).searchParams.get('customization'),'secondary');
  assert.ok(calls.every(args=>args.customization==='secondary'));
  assert.deepEqual(pageErrors,[]);
  console.log(JSON.stringify({passed:true,cases:['HTTP selection','cross-package rejection','MCP routing','viewer context and navigation']}));
} finally {
  if(browser)await browser.close();
  if(server.exitCode===null){const stopped=new Promise(resolve=>server.once('exit',resolve));server.kill();await stopped;}
  await fs.rm(workspace,{recursive:true,force:true});
}
