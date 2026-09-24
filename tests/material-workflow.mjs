import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { performance } from 'node:perf_hooks';

const root=path.resolve(import.meta.dirname,'..');
const base=process.env.APEX_VIEWER||'http://127.0.0.1:8765';
const scratch=path.join(root,'.apex/material-workflow');fs.mkdirSync(scratch,{recursive:true});
async function tool(name,args={},ok=true){
  const r=await fetch(new URL('/api/tool',base),{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({name,arguments:args})});
  const v=await r.json();assert.equal(r.ok,ok,JSON.stringify(v));return v;
}
const mode={id:'standard',primary:'M',phases:[{id:'work',work:10,requirements:[{resource:'M',retain:true}]}]};
const header={id:'synthetic-material-workflow',horizon:1000000,resources:[{id:'M',calendar:[{start:0,end:1000000}]}]};
const problem={...header,inventory:{raw:4,part:1},receipts:[{item:'part',at:15,amount:1}],tasks:[
  {id:'assemble',due:50,consume:{part:6},produce:{finished:2},modes:[mode]},
  {id:'make-part',consume:{blank:4},produce:{part:4},modes:[mode]},
  {id:'cut-blank',consume:{raw:4},produce:{blank:4},modes:[mode]}
]};
fs.writeFileSync(path.join(scratch,'input.json'),JSON.stringify(problem));
const imported=await tool('problem.import',{path:'.apex/material-workflow/input.json'});
const prepared=await tool('material.prepare',{scenario_id:imported.scenario_id,expected_revision:1});
assert.equal(prepared.added_dependencies,2);assert.equal(prepared.allocations,5);
const allocations=await tool('artifact.read',{artifact_id:prepared.report_id,pointer:'/allocations',limit:2});
assert.equal(allocations.items.length,2);assert.equal(allocations.next_offset,2);
const searches=[];
for(const method of ['schedule.create','schedule.plus','schedule.train']){
  const plan=await tool(method,{scenario_id:prepared.scenario_id,options:{iterations:16,budget_ms:0,trainer:{population_size:8,workers:2},plus:{workers:2,depth:3}}});
  assert.equal((await tool('schedule.validate',{schedule_id:plan.schedule_id})).valid,true);
  searches.push({method,schedule_id:plan.schedule_id,elapsed_ms:plan.elapsed_ms,metrics:plan.metrics});
}
const shortage=structuredClone(problem);shortage.inventory.raw=1;
const bad=await tool('problem.import',{problem:shortage});
const rejected=await tool('material.prepare',{scenario_id:bad.scenario_id,expected_revision:1},false);
assert.ok(rejected.diagnostics.some(d=>d.code==='MATERIAL_UNRESOLVED'));
const measurements=[];
for(const count of [1000,10000]){
  const p={...header,id:`synthetic-material-scale-${count}`,inventory:{raw:count},tasks:Array.from({length:count},(_,i)=>({id:`T${i}`,consume:{raw:1},modes:[mode]}))};
  const relative=`.apex/material-workflow/input-${count}.json`;fs.writeFileSync(path.join(root,relative),JSON.stringify(p));
  const source=await tool('problem.import',{path:relative});
  const samples=[];let value;
  for(let repeat=0;repeat<3;repeat++){
    const start=performance.now();value=await tool('material.prepare',{scenario_id:source.scenario_id,expected_revision:1});samples.push(performance.now()-start);
    assert.equal(value.allocations,count);assert.ok(JSON.stringify(value).length<1024);
  }
  const page=await tool('artifact.read',{artifact_id:value.report_id,pointer:'/allocations',limit:2});assert.equal(page.total,count);
  measurements.push({tasks:count,samples_ms:samples,median_ms:[...samples].sort((a,b)=>a-b)[1],response_bytes:Buffer.byteLength(JSON.stringify(value))});
}
const report={passed:true,scope:'Synthetic HTTP tool sequence exercising intake, material preparation, all planners, validation and bounded allocation reports; not an autonomous LLM evaluation.',timing:'Release service; HTTP round trip includes material checks, preparation, JSON persistence and new scenario validation. Three repeats; independent tasks with stock, not worst-case BOM networks.',searches,measurements};
fs.writeFileSync(path.join(root,'docs/reports/material-workflow.json'),JSON.stringify(report,null,2));
console.log(JSON.stringify(report));
