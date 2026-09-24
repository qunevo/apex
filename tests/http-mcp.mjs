import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js';
import { spawn } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const root=path.resolve(import.meta.dirname,'..');
const binary=process.env.APEX_BINARY||path.join(root,'target/release',process.platform==='win32'?'apex.exe':'apex');
const port=18765,base=`http://127.0.0.1:${port}`,token=randomBytes(24).toString('hex');
const process_=spawn(binary,['serve','--port',String(port),'--store',path.join(root,'.apex',`http-test-${Date.now()}`),'--workspace',root],{cwd:root,windowsHide:true,env:{...process.env,APEX_BIND:'127.0.0.1',APEX_API_TOKEN:token,APEX_PUBLIC_URL:base},stdio:'pipe'});
let stderr='';process_.stderr.on('data',d=>stderr+=d);
const headers={Authorization:`Bearer ${token}`};
const client=new Client({name:'apex-independent-http-agent',version:'1.0.0'});
const results=[];
async function call(name,args={}){const r=await client.callTool({name,arguments:args});assert.ok(!r.isError,JSON.stringify(r));return r.structuredContent||JSON.parse(r.content[0].text);}
try {
  let ready=false;
  for(let i=0;i<80;i++){try{const r=await fetch(base);if(r.ok){ready=true;break;}}catch{}await new Promise(r=>setTimeout(r,100));}
  assert.ok(ready,stderr);
  assert.equal((await fetch(`${base}/api/tools`)).status,401);
  assert.equal((await fetch(`${base}/api/tools`,{headers:{...headers,Origin:'https://untrusted.invalid'}})).status,403);
  assert.equal((await fetch(`${base}/mcp`,{headers})).status,405);
  assert.equal((await fetch(`${base}/mcp`,{method:'POST',headers:{...headers,'Content-Type':'application/json','MCP-Protocol-Version':'invalid'},body:'{}'})).status,400);
  const specification=await (await fetch(`${base}/openapi.json`)).json();
  assert.equal(Object.keys(specification.paths).length,32);
  const plain=await fetch(`${base}/api/tools/capabilities`,{method:'POST',headers:{...headers,'Content-Type':'application/json'},body:'{}'});
  assert.equal((await plain.json()).schema,'apex.v3.4');
  await client.connect(new StreamableHTTPClientTransport(new URL(`${base}/mcp`),{requestInit:{headers}}));
  assert.equal((await client.listTools()).tools.length,32);
  const schema=await call('schema.get',{model:'production'});assert.ok(schema.root.properties.demands);
  results.push({case:'official SDK Streamable HTTP, bearer token, origin checks, OpenAPI and plain JSON tools',passed:true});

  const imported=await call('production.import',{path:'examples/production-orders.json'});
  const baseline=await call('schedule.create',{scenario_id:imported.scenario_id});
  assert.equal(baseline.tasks,10);assert.equal(Object.keys(baseline.route_choices).length,5);
  const evolved=await call('schedule.evolve',{scenario_id:imported.scenario_id,schedule_id:baseline.schedule_id,options:{iterations:24,budget_ms:0,trainer:{population_size:4}}});
  assert.equal(evolved.search.algorithm,'direct_schedule_ga');assert.equal(evolved.evaluations,24);
  assert.equal((await call('schedule.validate',{schedule_id:evolved.schedule_id})).valid,true);
  const improved=await call('schedule.improve',{scenario_id:imported.scenario_id,schedule_id:baseline.schedule_id,options:{iterations:24,budget_ms:0,improve:{evolution_share:0.3},trainer:{population_size:4}}});
  assert.equal(improved.search.algorithm,'trainer_plus_ga');assert.equal(improved.search.phases.at(-1).name,'direct_ga');assert.equal(improved.search.operators.length,10);assert.equal(improved.evaluations,24);
  assert.equal((await call('schedule.validate',{schedule_id:improved.schedule_id})).valid,true);
  results.push({case:'combined improvement over HTTP MCP with current incumbent',passed:true,phases:improved.search.phases.map(p=>({name:p.name,evaluations:p.evaluations}))});
  const fork=await call('scenario.fork',{scenario_id:imported.scenario_id});
  const routes=await call('model.page',{scenario_id:fork.scenario_id,section:'routes',limit:1});
  const route=routes.items[0];
  const alternative=route.alternatives.find(a=>a.id!==baseline.route_choices[route.id]).id;
  await call('scenario.patch',{scenario_id:fork.scenario_id,expected_revision:1,patches:[{kind:'route_choice',route:route.id,alternative}]});
  const candidate=await call('schedule.train',{scenario_id:fork.scenario_id,options:{iterations:18,budget_ms:10000}});
  assert.equal(candidate.route_choices[route.id],alternative);
  const comparison=await call('scenario.compare',{baseline_id:baseline.schedule_id,candidate_id:candidate.schedule_id});
  assert.ok(comparison.removed_count>=2);
  const page=await call('schedule.page',{schedule_id:candidate.schedule_id,limit:2});
  assert.equal(Object.keys(page.order_completions).length,3);
  const detail=await call('task.inspect',{schedule_id:candidate.schedule_id,task:page.items[0].task});
  assert.ok(detail.task.source.includes('demand:'));
  assert.equal((await call('schedule.validate',{schedule_id:candidate.schedule_id})).valid,true);
  results.push({case:'quantity-based order expansion, whole-route change, mode conditionals, comparison and validation',passed:true,baseline:baseline.metrics,candidate:candidate.metrics,removed_tasks:comparison.removed_count,fast_ms:baseline.elapsed_ms,trainer_ms:candidate.elapsed_ms});

  await call('scenario.patch',{scenario_id:fork.scenario_id,expected_revision:2,patches:[{kind:'customization',customization:{id:'dummy_customer',version:'1'}}]});
  const decorated=await call('schedule.train',{scenario_id:fork.scenario_id,options:{iterations:12,budget_ms:10000}});
  assert.ok(decorated.metrics.dummy_priority_completion>0);
  assert.equal((await call('schedule.validate',{schedule_id:decorated.schedule_id})).valid,true);
  results.push({case:'versioned native sequence/objective extension through remote tools and independent revalidation',passed:true,metrics:decorated.metrics});

  const queues=await call('queues.inspect',{scenario_id:fork.scenario_id});
  assert.ok(queues.standard_queues.includes('deadline_interval_fit'));
  assert.ok(queues.mapping.some(m=>m.objective.metric==='dummy_priority_completion'));
  const searchOptions={iterations:32,budget_ms:0,improve:{evolution_share:0.3},trainer:{population_size:8,generations:2,workers:3,selection:'pareto',mutation_rate:1,crossover_rate:1},plus:{workers:2,depth:3,branching:4}};
  const evolution=await call('schedule.train',{scenario_id:fork.scenario_id,options:searchOptions});
  assert.equal(evolution.search.generations,2);assert.equal(evolution.search.evaluations,24);assert.equal(evolution.search.stop_reason,'generation_limit');assert.ok(evolution.search.mutations>0);assert.ok(evolution.search.crossovers>0);
  const elite=await call('schedule.create',{scenario_id:fork.scenario_id,options:evolution.replay});assert.deepEqual(elite.score,evolution.score);
  const plus=await call('schedule.plus',{scenario_id:fork.scenario_id,options:searchOptions});assert.equal(plus.search.algorithm,'uct_prefix_rollouts');assert.equal(plus.search.evaluations,32);assert.ok(plus.search.nodes>1);
  assert.equal((await call('schedule.validate',{schedule_id:plus.schedule_id})).valid,true);
  const freeze=await call('scenario.freeze',{scenario_id:fork.scenario_id,expected_revision:3,schedule_id:plus.schedule_id,until:1000,dimensions:['resource','order']});
  const frozenPlan=await call('schedule.create',{scenario_id:fork.scenario_id});
  const frozenPage=await call('schedule.page',{schedule_id:frozenPlan.schedule_id});
  assert.ok(frozenPage.freeze_zones.length>0);assert.ok(frozenPage.items.some(i=>i.locks.some(l=>l.kind==='resource')));
  const optionsSchema=await call('schema.get',{model:'options'});assert.ok(optionsSchema.root.properties.trainer);
  results.push({case:'evolution generations, mutation, crossover, parallel Pareto selection, replay, UCT tree and freeze annotations through official SDK',passed:true,trainer:{...evolution.search,archive:undefined},plus:{...plus.search,archive:undefined},frozen_tasks:freeze.frozen_tasks});
  fs.writeFileSync(path.join(root,'docs/reports/v0.6-http-workflows.json'),JSON.stringify({transport:'Streamable HTTP / official MCP SDK',engine:'0.6.0',results},null,2));
  console.log(JSON.stringify({passed:results.length,results},null,2));
} finally {await client.close();process_.kill();}
