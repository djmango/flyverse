// Minimal Chrome DevTools Protocol client over Node's built-in WebSocket.
// No npm dependencies: Node >= 22 ships a global WebSocket, so the whole video
// pipeline runs on the stock toolchain of this host.
//
// Only the four commands the pipeline needs are wrapped: attach to a page
// target, navigate, evaluate, and capture a PNG of the surface.

export class CDP {
  constructor(wsUrl, { timeoutMs = 120000 } = {}) {
    this.wsUrl = wsUrl;
    this.timeoutMs = timeoutMs;
    this.id = 0;
    this.pending = new Map();
    this.listeners = new Map();
    this.ws = null;
  }

  connect() {
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(this.wsUrl);
      this.ws = ws;
      const timer = setTimeout(() => reject(new Error('CDP connect timeout')), this.timeoutMs);
      ws.addEventListener('open', () => { clearTimeout(timer); resolve(this); });
      ws.addEventListener('error', (e) => { clearTimeout(timer); reject(new Error('CDP websocket error: ' + (e && e.message))); });
      ws.addEventListener('message', (ev) => {
        let msg;
        try { msg = JSON.parse(typeof ev.data === 'string' ? ev.data : String(ev.data)); } catch (_) { return; }
        if (msg.id !== undefined && this.pending.has(msg.id)) {
          const { resolve: res, reject: rej } = this.pending.get(msg.id);
          this.pending.delete(msg.id);
          if (msg.error) rej(new Error(msg.method + ' ' + JSON.stringify(msg.error)));
          else res(msg.result);
        } else if (msg.method) {
          const hs = this.listeners.get(msg.method);
          if (hs) hs.forEach((h) => { try { h(msg.params); } catch (_) { /* listener errors must not kill the stream */ } });
        }
      });
    });
  }

  on(method, handler) {
    if (!this.listeners.has(method)) this.listeners.set(method, []);
    this.listeners.get(method).push(handler);
  }

  send(method, params = {}) {
    const id = ++this.id;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error('CDP timeout: ' + method));
      }, this.timeoutMs);
      this.pending.set(id, {
        resolve: (v) => { clearTimeout(timer); resolve(v); },
        reject: (e) => { clearTimeout(timer); reject(e); },
      });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }

  /// Evaluate an expression in the page and return its JSON value. Throws if the
  /// page threw, so a broken step is loud rather than silently captured.
  async eval(expression) {
    const r = await this.send('Runtime.evaluate', {
      expression,
      returnByValue: true,
      awaitPromise: true,
      userGesture: true,
    });
    if (r.exceptionDetails) {
      const d = r.exceptionDetails;
      throw new Error('page exception: ' + (d.exception && d.exception.description ? d.exception.description : JSON.stringify(d)));
    }
    return r.result ? r.result.value : undefined;
  }

  close() { try { this.ws && this.ws.close(); } catch (_) { /* already gone */ } }
}

/// Poll an HTTP endpoint until it answers, or give up. Used to wait for the
/// freshly launched browser's DevTools HTTP server.
export async function waitForHttp(url, { tries = 200, delayMs = 100 } = {}) {
  for (let i = 0; i < tries; i++) {
    try {
      const res = await fetch(url);
      if (res.ok) return await res.json().catch(() => true);
    } catch (_) { /* not up yet */ }
    await new Promise((r) => setTimeout(r, delayMs));
  }
  throw new Error('endpoint never came up: ' + url);
}

/// Pick the page target a fresh browser starts with.
export async function firstPageTarget(cdpHttp) {
  const list = await waitForHttp(cdpHttp + '/json/list');
  const pages = (Array.isArray(list) ? list : []).filter((t) => t.type === 'page' && t.webSocketDebuggerUrl);
  if (!pages.length) throw new Error('no page target on ' + cdpHttp);
  return pages[0];
}