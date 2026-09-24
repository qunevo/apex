import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { performance } from 'node:perf_hooks';

const root = path.resolve(import.meta.dirname, '..');
const binary = process.env.APEX_BINARY || path.join(root, 'target/release', process.platform === 'win32' ? 'apex.exe' : 'apex');
const store = path.join(root, '.apex', `sdk-test-${Date.now()}`);
const client = new Client({ name: 'apex-integration-tests', version: '1.0.0' });
const transport = new StdioClientTransport({ command: binary, args: ['mcp', '--workspace', root, '--store', store], stderr: 'pipe' });
const results = [];
async function call(name, arguments_ = {}, expectedError = false) {
  const result = await client.callTool({ name, arguments: arguments_ });
  assert.equal(Boolean(result.isError), expectedError, `${name}: ${JSON.stringify(result)}`);
  return result.structuredContent || JSON.parse(result.content[0].text);
}
try {
  await client.connect(transport);
  const tools = await client.listTools();
  assert.equal(tools.tools.length, 32);
  const caps = await call('capabilities');
  assert.equal(caps.engine, 'rust');
  const schema = await call('schema.get', { definition: 'Task' });
  assert.ok(schema.properties.modes);
  results.push({ case: 'official SDK handshake, schema discovery and 32 tools', passed: true });

  const imported = await call('problem.import', { path: 'examples/shift-factory.json' });
  const baseline = await call('schedule.create', { scenario_id: imported.scenario_id });
  assert.equal(baseline.valid, true);
  const evolved=await call('schedule.evolve',{scenario_id:imported.scenario_id,schedule_id:baseline.schedule_id,options:{iterations:24,budget_ms:0,trainer:{population_size:4}}});
  assert.equal(evolved.search.algorithm,'direct_schedule_ga');assert.equal(evolved.evaluations,24);
  assert.equal((await call('schedule.validate',{schedule_id:evolved.schedule_id})).valid,true);
  const improved = await call('schedule.improve', { scenario_id: imported.scenario_id, schedule_id:baseline.schedule_id, options:{iterations:20,budget_ms:0,improve:{evolution_share:0.3},trainer:{population_size:4}} });
  assert.equal(improved.search.algorithm,'trainer_plus_ga');assert.equal(improved.search.phases.at(-1).name,'direct_ga');assert.equal(improved.search.operators.length,10);
  assert.equal(improved.evaluations,20);
  assert.ok(improved.search.phases.length>1);
  assert.equal((await call('schedule.validate',{schedule_id:improved.schedule_id})).valid,true);
  results.push({case:'combined improvement with pinned incumbent and shared budget',passed:true,phases:improved.search.phases.map(p=>({name:p.name,evaluations:p.evaluations})),score:improved.score});
  const frozen = await call('scenario.freeze', { scenario_id: imported.scenario_id, expected_revision: 1, schedule_id: baseline.schedule_id, until: 1000, dimensions: ['resource', 'order'] });
  assert.ok(frozen.frozen_tasks > 0);
  const fork = await call('scenario.fork', { scenario_id: imported.scenario_id });
  await call('scenario.patch', { scenario_id: fork.scenario_id, expected_revision: 1, patches: [{ kind: 'downtime', resource: 'CUT0', start: 0, end: 900, forbid_retention: true }] });
  const repaired = await call('schedule.repair', { scenario_id: fork.scenario_id, options: { iterations: 6, budget_ms: 10000 } });
  const comparison = await call('scenario.compare', { baseline_id: baseline.schedule_id, candidate_id: repaired.schedule_id });
  assert.ok(comparison.changed_count > 0);
  assert.equal((await call('schedule.validate', { schedule_id: repaired.schedule_id })).valid, true);
  results.push({ case: 'import, freeze, fork, downtime, repair, compare and validate', passed: true, baseline: baseline.metrics, candidate: repaired.metrics, changed_tasks: comparison.changed_count });

  await call('scenario.patch', { scenario_id: fork.scenario_id, expected_revision: 2, patches: [{ kind: 'rules', rules: [{ kind: 'attribute_objective', id: 'urgency_completion', attribute: 'urgency', weight: 1, priority: 0 }] }] });
  const trained = await call('schedule.train', { scenario_id: fork.scenario_id, options: { iterations: 6, budget_ms: 10000 } });
  assert.ok(trained.metrics.urgency_completion > 0);
  results.push({ case: 'agent-defined objective and compiled dispatch signal', passed: true, selected_strategy: trained.selected_strategy, metrics: trained.metrics });
  await call('scenario.patch', { scenario_id: fork.scenario_id, expected_revision: 1, patches: [] }, true);
  await call('solver.solve', {}, true);

  const governed = await call('problem.import', {path:'examples/dispatch-campaign.json'});
  const inspected = await call('policy.inspect', {scenario_id:governed.scenario_id});
  assert.equal(inspected.planning.policies[0].id, 'six-productive-hours');
  const governedPlan = await call('schedule.create', {scenario_id:governed.scenario_id});
  const explanation = await call('schedule.explain_decision', {schedule_id:governedPlan.schedule_id,task:'A-02'});
  assert.equal(explanation.enabled,true);assert.ok(explanation.step.reasons.length>0);
  const blocked = await call('schedule.create',{scenario_id:governed.scenario_id,options:{decision_prefix:[{task:'B-01',mode:'on-M0'}]}},true);
  assert.ok(blocked.diagnostics.some(d=>d.code==='DISPATCH_PREFIX'));
  for (const name of ['schedule.train','schedule.plus']) {
    const result=await call(name,{scenario_id:governed.scenario_id,options:{iterations:8,budget_ms:0}});
    assert.equal((await call('schedule.validate',{schedule_id:result.schedule_id})).valid,true);
  }
  results.push({case:'declarative campaign, immutable explanations, blocked prefix and both searches through MCP',passed:true});
  const count = process.argv.includes('--large') ? 100000 : 2000;
  const template = JSON.parse(fs.readFileSync(path.join(root, 'examples/demo.json'), 'utf8'));
  const taskTemplate = template.tasks[0];
  template.tasks = [];
  template.horizon = Math.max(86400, count * 1000);
  for (const r of template.resources) r.calendar[0].end = template.horizon;
  const started = performance.now();
  const session = await call('import.begin', { problem: template, expected_tasks: count });
  const chunkPath = path.join(store, 'input-chunk.json');
  let totalInputBytes = 0;
  let maxResponseBytes = 0;
  for (let offset = 0; offset < count; offset += 2000) {
    const rows = Array.from({ length: Math.min(2000, count - offset) }, (_, j) => ({ ...taskTemplate, id: `B${offset + j}`, source: `synthetic:row:${offset + j}` }));
    const text = JSON.stringify(rows);
    totalInputBytes += Buffer.byteLength(text);
    fs.writeFileSync(chunkPath, text);
    const args = { import_id: session.import_id, chunk_id: `chunk-${offset}`, path: path.relative(root, chunkPath) };
    const response = await call('import.append', args);
    maxResponseBytes = Math.max(maxResponseBytes, Buffer.byteLength(JSON.stringify(response)));
    if (offset === 0) assert.equal((await call('import.append', args)).replayed, true);
  }
  const finalized = await call('import.finalize', { import_id: session.import_id });
  const page = await call('tasks.page', { scenario_id: finalized.scenario_id, limit: 3 });
  assert.equal(page.total, count);
  assert.equal(page.items.length, 3);
  results.push({ case: 'large artifact-based chunk import with bounded responses', passed: true, tasks: count, elapsed_ms: performance.now() - started, total_input_bytes: totalInputBytes, max_append_response_bytes: maxResponseBytes });
  fs.mkdirSync(path.join(root, 'docs/reports'), { recursive: true });
  fs.writeFileSync(path.join(root, process.env.APEX_REPORT || 'docs/reports/v0.5-agent-workflows.json'), JSON.stringify({ sdk: 'official @modelcontextprotocol/sdk', engine: caps.version, results }, null, 2));
  console.log(JSON.stringify({ passed: results.length, results }, null, 2));
} finally {
  await client.close();
}
