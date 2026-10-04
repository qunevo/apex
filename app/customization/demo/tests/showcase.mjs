// Deliberately synthetic, small factory for the shipped MCP App integration test.
const stages = ['Machining', 'Assembly', 'Testing'];
const machines = [['CNC-01', 'CNC-02'], ['ASSEMBLY-01', 'ASSEMBLY-02'], ['TEST-01', 'TEST-02']];
const jobs = Array.from({ length: 12 }, (_, i) => ({ id: `LOT-${String(i + 1).padStart(3, '0')}`, item: `ITEM-${i % 3 + 1}`, quantity: 1, due: i < 3 ? 7000 + i * 2200 : 25000 + i * 3500, priority: i < 2 ? 3 : i < 5 ? 2 : 1 }));
export const facts = {
  id: 'synthetic-insights-showcase', epoch: '2026-10-05T06:00:00Z', horizon: 172800,
  resources: machines.flat().map(id => ({ id, capacity: 1, calendar: [{ start: 0, end: 172800 }] })), jobs,
  orders: jobs.map((job, i) => ({ id: `ORDER-${String(i + 1).padStart(3, '0')}`, jobs: [job.id], due: job.due, priority: job.priority })),
  tasks: jobs.flatMap((job, i) => stages.map((stage, s) => ({
    id: `${job.id}:${s + 1}`, job: job.id, family: job.item, stage, priority: job.priority, due: job.due,
    modes: machines[s].map((resource, m) => ({ id: resource, primary: resource,
      phases: [{ id: 'run', work: 2100 + i % 4 * 450 + s * 120 - m * 90, requirements: [{ resource, retain: true }] }] })),
  }))),
  dependencies: jobs.flatMap(job => [1, 2].map(step => ({ before: `${job.id}:${step}`, after: `${job.id}:${step + 1}` }))),
};
export const sourceSummary = {
  sources: [{ label: 'MES', revision: '12', sha256: 'a'.repeat(64) }, { label: 'Excel', sha256: 'b'.repeat(64) }],
  counts: { mes_operations: 42, planned_operations: 36, excel_operations: 38, closed_operations: 6, operations_without_excel: 4, resources: 6 },
  notices: [{ code: 'SYNTHETIC_SOURCE_NOTE', message: 'Four released operations have no workbook proposal and remain available for planning.' }], notice_count: 1,
};
