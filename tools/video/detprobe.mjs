// Determinism probe: drive the page to one fixed frame and screenshot it twice
// without changing any state. If the two PNGs differ, the non-determinism is in
// the rasteriser, not in the replay state.
import { writeFileSync } from 'node:fs';
import { CDP, firstPageTarget } from './cdp.mjs';

const cdpHttp = process.argv[2] || 'http://127.0.0.1:9333';
const pageUrl = process.argv[3] || 'http://127.0.0.1:8137/index.html?skip=brain,spikes';
const out = process.argv[4] || '/tmp/fvsrv/det-probe';

const t = await firstPageTarget(cdpHttp);
const cdp = new CDP(t.webSocketDebuggerUrl);
await cdp.connect();
await cdp.send('Page.enable');
await cdp.send('Runtime.enable');
await cdp.send('Emulation.setDeviceMetricsOverride', { width: 1600, height: 900, deviceScaleFactor: 1, mobile: false });
const loaded = new Promise((r) => cdp.on('Page.loadEventFired', r));
await cdp.send('Page.navigate', { url: pageUrl });
await loaded;
for (let i = 0; i < 600; i++) {
  if (await cdp.eval('!!(window.__flyverse && window.__flyverse.anim && window.__flyverse.anim.loaded)')) break;
  await new Promise((r) => setTimeout(r, 100));
}
const payload = JSON.stringify({
  seq: 0, sim_ms: 0, wall_rt: null, paused: false, stimulus: 'probe',
  body: { pos: [-220, -150, 0], quat: [1, 0, 0, 0], mode: 'GROUND', airborne: false, speed: 0, pitch: 0, roll: 0, yaw: 0.6, wing_amp: 0.3 },
  motors: {}, neural: {}, events: {},
});
await cdp.eval('window.__flyverse.replay.start()');
for (let i = 0; i < 30; i++) await cdp.eval('window.__flyverse.replay.step(0.5,0.02,' + payload + ')');

for (let i = 0; i < 3; i++) {
  const s = await cdp.send('Page.captureScreenshot', { format: 'png', fromSurface: true });
  writeFileSync(out + '-repeat' + i + '.png', Buffer.from(s.data, 'base64'));
}

// Same frame rendered after an explicit extra render() call with no state change.
await cdp.eval('window.__flyverse.replay.step(0.5,0.02,' + payload + ')');
const s2 = await cdp.send('Page.captureScreenshot', { format: 'png', fromSurface: true });
writeFileSync(out + '-after-step.png', Buffer.from(s2.data, 'base64'));
cdp.close();
console.log('probe written');