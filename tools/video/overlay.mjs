// The on-screen evidence panel for the replay video, injected into the page by
// the driver. Kept in its own file so the overlay's wording is reviewable apart
// from the capture machinery.
//
// Sourcing rule, applied to every number on screen: it is either a field of the
// run's summary.json, or a column of the run's trace.csv, or an arithmetic
// difference of two trace columns that the label names. Nothing is invented and
// nothing is smoothed for display.
//
// Defines window.__fvev = { series(s), meta(m), set(sample), errors() }.

export const OVERLAY_JS = String.raw`(function () {
  var F = window.__fvev = {};
  F._err = [];
  window.addEventListener('error', function (e) {
    F._err.push(String((e && e.message) || e));
  }, true);
  F.errors = function () { return F._err.slice(); };

  var MONO = 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace';

  var root = document.createElement('div');
  root.id = 'fvev';
  root.style.cssText = [
    'position:fixed', 'top:8px', 'right:8px', 'width:452px', 'z-index:900',
    'background:rgba(8,12,17,0.90)', 'border:1px solid #1e2a35', 'border-radius:3px',
    'padding:7px 9px 9px', 'font:' + '11px/1.5 ' + MONO, 'color:#c8d6e0',
    'pointer-events:none', 'white-space:nowrap',
  ].join(';');
  document.body.appendChild(root);

  function head(txt, colour) {
    var h = document.createElement('div');
    h.textContent = txt;
    h.style.cssText = 'margin:0 0 5px;font-size:9px;letter-spacing:0.18em;color:' +
      (colour || '#6c8090') + ';font-weight:600;border-bottom:1px solid #1e2a35;padding-bottom:3px';
    return h;
  }
  function hideRow(el) { if (el) el.style.display = 'none'; }
  function showRow(el) { if (el) el.style.display = 'flex'; }
  function rows(parent, defs) {
    var out = {};
    defs.forEach(function (d) {
      var r = document.createElement('div');
      r.style.cssText = 'display:flex;justify-content:space-between;gap:8px';
      var k = document.createElement('span');
      k.textContent = d[0];
      k.style.cssText = 'color:' + (d[2] || '#6c8090');
      var v = document.createElement('span');
      v.textContent = d[1] || '-';
      v.style.cssText = 'color:#c8d6e0;font-variant-numeric:tabular-nums';
      r.appendChild(k); r.appendChild(v);
      parent.appendChild(r);
      out[d[3] || d[0]] = v;
    });
    return out;
  }

  // ---- run identity (whole panel is one run) ----
  root.appendChild(head('RUN EVIDENCE'));
  var id = rows(root, [
    ['run dir',          '-', null, 'run'],
    ['seed',             '-', null, 'seed'],
    ['sim seconds',      '-', null, 'seconds'],
    ['room',             '-', null, 'room'],
    ['target / food place', '-', null, 'target'],
    ['palm',             '-', null, 'palm'],
    ['pack',             '-', null, 'pack'],
    ['sample every',     '-', null, 'every'],
    ['pack/room source', 'summary.json config', '#8a7550', 'src'],
  ]);
  // The palm row exists only for a run that recorded a hand (hand.json /
  // summary.hand). It is hidden otherwise, so a run with no hand is laid out
  // exactly as before this row was added.
  hideRow(id.palm.parentNode);

  var scaleLine = document.createElement('div');
  scaleLine.style.cssText = 'margin:5px 0 4px;color:#ffcf4a;font-weight:600';
  root.appendChild(scaleLine);

  var label = document.createElement('div');
  label.style.cssText = 'margin:0 0 5px;color:#3ad0ff';
  root.appendChild(label);

  // ---- live read-outs ----
  root.appendChild(head('LIVE  (trace.csv column named in brackets)'));
  var live = rows(root, [
    ['sim t',                  '-', null, 't'],
    ['mode [mode]',            '-'],
    ['airborne [mode]',        '-'],
    ['altitude [z]',           '-'],
    ['speed [speed]',          '-'],
    ['yaw rate [yaw_rate]',    '-'],
    ['steering diff',          '-'],
    ['wing amp [wing_amp]',    '-'],
    ['power L / R [pow_l,pow_r]', '-', null, 'power'],
    ['steer L / R [steer_l,r]',   '-', null, 'steer'],
    ['wall dist [wall_dist]',  '-'],
    ['walls / takeoffs / landings', '-', null, 'events'],
    ['loom [loom]',            '-'],
    ['odor L / R [odor_l,r]',  '-', null, 'odor'],
    ['flow L / R [flow_l,r]',  '-', null, 'flow'],
    ['spikes win / total',     '-', null, 'spikes'],
    ['fruit dist / cols [fruit_dist,fruit_cols]', '-', null, 'fruit'],
    ['palm dist / surf [hand.csv]', '-', null, 'palm'],
  ]);
  // Both rows are hidden unless the run recorded the thing they report: the
  // fruit columns are in trace.csv, the palm's distance is in hand.csv (the
  // hand's own file -- the trace has no hand columns).
  hideRow(live.fruit.parentNode);
  hideRow(live.palm.parentNode);

  // ---- sparkline: recorded speed and steering differential vs sim time ----
  root.appendChild(head('TRACE OVER SIM TIME'));
  var cv = document.createElement('canvas');
  cv.width = 436; cv.height = 74;
  cv.style.cssText = 'display:block;width:436px;height:74px;background:#05070a;border:1px solid #1e2a35';
  root.appendChild(cv);
  var legend = document.createElement('div');
  legend.textContent = 'cyan = speed [speed, mm/s]   amber = steering differential [steer_l - steer_r]   white = playhead';
  legend.style.cssText = 'color:#6c8090;font-size:9px;margin-top:3px;white-space:normal';
  root.appendChild(legend);

  var legend2 = document.createElement('div');
  legend2.style.cssText = 'color:#6c8090;font-size:9px;margin-top:4px;white-space:normal';
  root.appendChild(legend2);

  // ---- top-down path over the whole run, in the arena's own frame ----
  root.appendChild(head('HORIZONTAL PATH, WHOLE RUN  (trace x,y vs arena)'));
  var cv2 = document.createElement('canvas');
  cv2.width = 436; cv2.height = 132;
  cv2.style.cssText = 'display:block;width:436px;height:132px;background:#05070a;border:1px solid #1e2a35';
  root.appendChild(cv2);
  var legend3 = document.createElement('div');
  legend3.textContent = 'grey = arena wall [summary.config.arena_mm]   cyan = path   amber dot = current position';
  legend3.style.cssText = 'color:#6c8090;font-size:9px;margin-top:3px;white-space:normal';
  root.appendChild(legend3);

  var series = null;
  F.series = function (s) { series = s; };

  F.meta = function (m) {
    id.run.textContent = m.run;
    id.seed.textContent = String(m.seed);
    id.seconds.textContent = m.seconds + ' s';
    id.room.textContent = m.room;
    id.target.textContent = m.target;
    if (m.palmText) {
      id.palm.textContent = m.palmText;
      showRow(id.palm.parentNode);
    }
    if (m.fruitLive) showRow(live.fruit.parentNode);
    if (m.handLive) showRow(live.palm.parentNode);
    id.pack.textContent = m.pack;
    id.every.textContent = m.every;
    scaleLine.textContent = 'TIME SCALE: 1 s sim = ' + m.timeScale.toFixed(2) + ' s video   (' +
      m.fps + ' fps, ' + m.frames + ' frames, ' + m.videoSeconds.toFixed(1) + ' s)';
    label.textContent = m.label || '';
    label.style.display = m.label ? 'block' : 'none';
    legend2.textContent = m.flags;
  };

  var launched = null, finalised = null;
  var submitted = null;

  F.set = function (s) {
    live.t.textContent = s.t.toFixed(3) + ' s';
    live['mode [mode]'].textContent = s.mode;
    live['airborne [mode]'].textContent = s.airborne ? 'yes' : 'no';
    live['altitude [z]'].textContent = s.z.toFixed(1) + ' mm';
    live['speed [speed]'].textContent = s.speed.toFixed(1) + ' mm/s';
    live['yaw rate [yaw_rate]'].textContent = s.yawRate.toFixed(3) + ' rad/s';
    live['steering diff'].textContent = s.steerDiff.toFixed(4);
    live['wing amp [wing_amp]'].textContent = s.wingAmp.toFixed(3);
    live.power.textContent = s.powL.toFixed(3) + ' / ' + s.powR.toFixed(3);
    live.steer.textContent = s.steerL.toFixed(3) + ' / ' + s.steerR.toFixed(3);
    live['wall dist [wall_dist]'].textContent = s.wallDist.toFixed(1) + ' mm';
    live.events.textContent = s.wallHits + ' / ' + s.takeoffs + ' / ' + s.landings;
    live['loom [loom]'].textContent = s.loom.toFixed(3);
    live.odor.textContent = s.odorL.toFixed(3) + ' / ' + s.odorR.toFixed(3);
    live.flow.textContent = s.flowL.toFixed(3) + ' / ' + s.flowR.toFixed(3);
    live.spikes.textContent = s.winSpikes + ' / ' + s.totSpikes;
    // Fruit and palm rows: written only when this frame carries the value, and
    // the rows are only shown for runs whose files hold them (see F.meta).
    if (typeof s.fruitDist === 'number') {
      live.fruit.textContent = s.fruitDist.toFixed(1) + ' mm / ' + s.fruitCols;
    }
    if (typeof s.palmDist === 'number') {
      live.palm.textContent = s.palmDist.toFixed(1) + ' / ' + s.palmSurf.toFixed(1) + ' mm';
    }
    F._curX = s.x; F._curY = s.y;
    draw(s.t);
  };

  function draw(tNow) {
    var g = cv.getContext('2d');
    var W = cv.width, H = cv.height;
    g.clearRect(0, 0, W, H);
    if (!series || !series.t.length) return;
    var t0 = series.t[0], t1 = series.t[series.t.length - 1];
    var span = Math.max(1e-6, t1 - t0);
    var smax = 1e-6, dmax = 1e-6;
    for (var i = 0; i < series.speed.length; i++) {
      smax = Math.max(smax, series.speed[i]);
      dmax = Math.max(dmax, Math.abs(series.steerDiff[i]));
    }
    function X(t) { return ((t - t0) / span) * (W - 2) + 1; }
    function plot(arr, scale, colour) {
      g.strokeStyle = colour; g.lineWidth = 1; g.beginPath();
      for (var i = 0; i < arr.length; i++) {
        var x = X(series.t[i]);
        var y = H - 2 - (Math.abs(arr[i]) / scale) * (H - 6);
        if (i === 0) g.moveTo(x, y); else g.lineTo(x, y);
      }
      g.stroke();
    }
    plot(series.speed, smax, '#3ad0ff');
    plot(series.steerDiff, dmax, '#ffa33c');
    var xp = X(Math.min(Math.max(tNow, t0), t1));
    g.strokeStyle = '#ffffff'; g.lineWidth = 1;
    g.beginPath(); g.moveTo(xp, 0); g.lineTo(xp, H); g.stroke();
    g.fillStyle = '#6c8090'; g.font = '9px ' + MONO;
    g.fillText(smax.toFixed(0), 2, 9);
    g.fillText('0', 2, H - 2);

    // top-down path in the arena's own frame
    var g2 = cv2.getContext('2d');
    var W2 = cv2.width, H2 = cv2.height;
    g2.clearRect(0, 0, W2, H2);
    var A = series.arena || { x: [-300, 300], y: [-220, 220] };
    var padp = 4;
    var sxp = (W2 - 2 * padp) / Math.max(1e-6, A.x[1] - A.x[0]);
    var syp = (H2 - 2 * padp) / Math.max(1e-6, A.y[1] - A.y[0]);
    var sc = Math.min(sxp, syp);
    var oxm = (A.x[0] + A.x[1]) / 2, oym = (A.y[0] + A.y[1]) / 2;
    function PX(x) { return W2 / 2 + (x - oxm) * sc; }
    function PY(y) { return H2 / 2 - (y - oym) * sc; }
    g2.strokeStyle = '#3a4d5e'; g2.lineWidth = 1;
    g2.strokeRect(PX(A.x[0]), PY(A.y[1]), (A.x[1] - A.x[0]) * sc, (A.y[1] - A.y[0]) * sc);
    g2.strokeStyle = '#3ad0ff'; g2.lineWidth = 1; g2.beginPath();
    for (var j = 0; j < series.x.length; j++) {
      var px = PX(series.x[j]), py = PY(series.y[j]);
      if (j === 0) g2.moveTo(px, py); else g2.lineTo(px, py);
    }
    g2.stroke();
    if (typeof F._curX === 'number') {
      g2.fillStyle = '#ffa33c';
      g2.beginPath();
      g2.arc(PX(F._curX), PY(F._curY), 2.5, 0, Math.PI * 2);
      g2.fill();
    }
  }
  F.draw = draw;

  return true;
})()`;