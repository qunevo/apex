// MCP Apps 2026-01-26: a small host transport with no network or stored tokens.
window.apexHost = (() => {
  const pending = new Map();
  const handlers = new Map();
  let nextId = 0;
  let origin;
  let closed = false;
  function send(message) {
    // A sandbox proxy may have an opaque origin; source identity is checked below.
    parent.postMessage({ jsonrpc: '2.0', ...message }, origin && origin !== 'null' ? origin : '*');
  }
  function request(method, params) {
    if (closed) return Promise.reject(new Error('The host connection is closed'));
    const id = ++nextId;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { pending.delete(id); reject(new Error('The host did not respond')); }, 15000);
      pending.set(id, { resolve, reject, timer });
      send({ id, method, params });
    });
  }
  function close() {
    closed = true;
    window.removeEventListener('message', receive);
    for (const p of pending.values()) { clearTimeout(p.timer); p.reject(new Error('The host connection is closed')); }
    pending.clear();
  }
  function receive(event) {
    if (event.source !== parent || (origin !== undefined && event.origin !== origin)) return;
    const message = event.data;
    if (!message || message.jsonrpc !== '2.0') return;
    if (message.method) {
      if ('id' in message) {
        if (message.method === 'ping' || message.method === 'ui/resource-teardown') {
          send({ id: message.id, result: {} });
          if (message.method === 'ui/resource-teardown') { handlers.get('teardown')?.(); close(); }
        } else send({ id: message.id, error: { code: -32601, message: 'Method not found' } });
      } else handlers.get(message.method)?.(message.params ?? {});
    } else if (pending.has(message.id)) {
      origin ??= event.origin;
      const p = pending.get(message.id);
      pending.delete(message.id);
      clearTimeout(p.timer);
      if (message.error) p.reject(new Error(message.error.message ?? 'Host request failed'));
      else p.resolve(message.result);
    }
  }
  window.addEventListener('message', receive);
  return {
    request,
    notify: (method, params = {}) => { if (!closed) send({ method, params }); },
    on: (method, handler) => handlers.set(method, handler),
    async connect() {
      if (parent === window) throw new Error('Open this view through results.get in an MCP Apps host');
      const result = await request('ui/initialize', {
        appInfo: { name: 'APEX Plan', version: '1.0.0' },
        appCapabilities: { availableDisplayModes: ['inline'] }, protocolVersion: '2026-01-26',
      });
      if (result.protocolVersion !== '2026-01-26') throw new Error('Unsupported MCP Apps protocol version');
      return result;
    },
  };
})();
