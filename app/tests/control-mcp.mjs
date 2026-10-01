// The released middleware binary, official MCP SDK and configured package workflow.
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js';
import { spawn } from 'node:child_process';
import { createHash, randomBytes, randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';

const root = path.resolve(import.meta.dirname, '..');
const binary = process.env.APEX_CONTROL_BINARY || path.join(root, 'target/release', process.platform === 'win32' ? 'apex-control.exe' : 'apex-control');
const directory = await mkdtemp(path.join(tmpdir(), 'apex-control-test-'));
const reservation = createServer().listen(0, '127.0.0.1');
await once(reservation, 'listening');
const port = reservation.address().port;
await new Promise(resolve => reservation.close(resolve));
const url = new URL(`http://127.0.0.1:${port}/mcp`);
const token = randomBytes(24).toString('hex');
const tenant = randomUUID();
const auth = path.join(directory, 'auth.json');
await writeFile(auth, JSON.stringify({ tenants: [{ id: tenant, name: 'Synthetic test' }], tokens: [{ sha256: createHash('sha256').update(token).digest('hex'), tenant, actor: 'test-agent', roles: ['admin'] }] }));
const env = { ...process.env };
delete env.APEX_CONTROL_DATABASE_URL;
const server = spawn(binary, ['serve', '--auth', auth, '--config', path.join(root, 'apex.config.json'), '--bind', `127.0.0.1:${port}`, '--workers', '1'], { cwd: directory, windowsHide: true, env, stdio: 'pipe' });
let diagnostics = '';
server.stderr.on('data', data => { diagnostics += data; });
server.stdout.resume();
const client = new Client({ name: 'apex-control-sdk-test', version: '1.0.0' });
async function call(name, args) {
  const result = await client.callTool({ name, arguments: args });
  assert.ok(!result.isError, JSON.stringify(result));
  return result.structuredContent;
}
try {
  let ready = false;
  for (let i = 0; i < 100; i++) {
    if (server.exitCode !== null) throw new Error(diagnostics);
    try { ready = (await fetch(new URL('/health', url))).ok; } catch {}
    if (ready) break;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  assert.ok(ready, diagnostics);
  assert.equal((await fetch(url, { method: 'POST' })).status, 401);
  await client.connect(new StreamableHTTPClientTransport(url, { requestInit: { headers: { Authorization: `Bearer ${token}` } } }));
  const tools = await client.listTools();
  const ui = tools.tools.find(t => t.name === 'results.get')._meta.ui.resourceUri;
  const resources = await client.listResources();
  assert.ok(resources.resources.some(resource => resource.uri === ui));
  const resource = await client.readResource({ uri: ui });
  assert.equal(resource.contents[0].mimeType, 'text/html;profile=mcp-app');
  assert.ok(resource.contents[0].text.includes('ui/initialize'));
  const facts = JSON.parse(await readFile(path.join(root, 'examples/shift-factory.json'), 'utf8'));
  const created = await call('scenarios.create', { name: 'Synthetic SDK scenario', engine: 'apex', content: { facts } });
  const scenario = created.scenario.id;
  const revision = await call('revisions.get', { scenario_id: scenario, number: 1 });
  assert.deepEqual(revision.content.customization_package, { id: 'demo', version: '1' });
  const queued = await call('runs.start', { scenario_id: scenario });
  let run;
  for (let i = 0; i < 100; i++) {
    run = await call('runs.get', { run_id: queued.id });
    if (['succeeded', 'failed', 'cancelled'].includes(run.state)) break;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  assert.equal(run.state, 'succeeded', JSON.stringify(run));
  const result = await call('results.get', { result_id: run.result });
  assert.equal(result.validation.valid, true);
  assert.deepEqual(result.provenance.customization_package, revision.content.customization_package);
  assert.equal(result.provenance.content_hash, revision.content_hash);
  assert.ok(result.view.operations.length > 0);
  console.log('Control MCP: release startup, auth, official SDK discovery, UI resource, config, worker and result provenance passed');
} finally {
  await client.close();
  const stopped = once(server, 'exit');
  if (server.exitCode === null) { server.kill(); await stopped; }
  assert.ok(path.resolve(directory).startsWith(path.resolve(tmpdir()) + path.sep));
  await rm(directory, { recursive: true, force: true });
}
