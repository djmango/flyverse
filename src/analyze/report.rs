//! Human-facing output: the console headline, the CSV trace dump, and the
//! self-contained HTML report.

use anyhow::Result;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::trace::Sample;

pub(crate) fn headline(sum: &serde_json::Value) -> String {
    let num = |a: &str, b: &str| -> f64 { sum[a][b].as_f64().unwrap_or(0.0) };
    format!(
        "BEHAVIOUR\n  mode %      ground {:.1}  takeoff {:.1}  cruise {:.1}  landing {:.1}  feeding {:.1}\n  \
         events       takeoffs {}  landings {}  wall hits {} ({:.1}/min, one every {:.1}s)  eats {}\n  \
         movement     path {:.0} mm, net {:.0} mm, tortuosity {:.2}, mean speed {:.0} mm/s (p95 {:.0})\n  \
         altitude     mean {:.1} mm, p05 {:.1}, p95 {:.1}, max {:.1}\n  \
         walls        {:.1}% of airborne samples within 20 mm of a wall\n  \
         turning      {:.2} full circles airborne, mean |yaw rate| {:.2} rad/s, {:.1}% of time turning > 0.5\n  \
         LOOM TEST    corr(loom, steer diff) = {:.3}, corr(loom, yaw rate) = {:.3}\n  \
         wall ahead   |yaw rate| {:.2} rad/s when looming vs {:.2} when calm; |steer| {:.3} vs {:.3}\n  \
         neural       {:.0} spikes/s of sim, {:.2} Hz mean rate",
        num("mode_percent","GROUND"),
        num("mode_percent","TAKEOFF"),
        num("mode_percent","CRUISE"),
        num("mode_percent","LANDING"),
        num("mode_percent","FEEDING"),
        num("events","takeoffs"),
        num("events","landings"),
        num("events","wall_hits"),
        num("events","wall_hits_per_min"),
        num("events","mean_seconds_between_wall_hits"),
        num("events","eats"),
        num("movement","path_horizontal_mm"),
        num("movement","net_horizontal_displacement_mm"),
        num("movement","tortuosity"),
        num("movement","mean_speed_mm_s"),
        num("movement","p95_speed_mm_s"),
        num("altitude_mm","mean"),
        num("altitude_mm","p05"),
        num("altitude_mm","p95"),
        num("altitude_mm","max"),
        num("walls","pct_airborne_within_20mm"),
        num("turning","full_circles_airborne"),
        num("turning","mean_abs_yaw_rate_rad_s"),
        num("turning","pct_airborne_turning_gt_0p5"),
        num("loom_response","pearson_loom_vs_steer_differential"),
        num("loom_response","pearson_loom_vs_yaw_rate"),
        num("loom_response","mean_abs_yaw_rate_when_loom_gt_0p5"),
        num("loom_response","mean_abs_yaw_rate_when_loom_lt_0p2"),
        num("loom_response","mean_abs_steer_when_loom_gt_0p5"),
        num("loom_response","mean_abs_steer_when_loom_lt_0p2"),
        num("neural","spikes_per_sim_second"),
        num("neural","mean_rate_hz"),
    )
}

pub(crate) fn write_csv(path: &PathBuf, s: &[Sample]) -> Result<()> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    writeln!(
        f,
        "t,x,y,z,speed,yaw,yaw_rate,roll,pitch,wing_amp,mode,pow_l,pow_r,steer_l,steer_r,\
walk_l,walk_r,land_l,land_r,mn9,dn02,dn07,sapp,land_dn,dn_filt,odor_l,odor_r,flow_l,flow_r,\
loom,taste,wall_dist,tau_roll,tau_pitch,tau_yaw,damp_roll,damp_pitch,damp_yaw,wroll,wpitch,\
wyaw,tilt_l,tilt_r,tilt_deg,fz_body,fz_world,steer_l_hz,steer_r_hz,touch_x,touch_y,touch_z,\
tau_yaw_tilt,tau_yaw_amp,wall_hits,takeoffs,landings,eats,win_spikes,tot_spikes"
    )?;
    for x in s {
        writeln!(
            f,
            "{:.3},{:.2},{:.2},{:.2},{:.2},{:.4},{:.4},{:.4},{:.4},{:.4},{},{:.4},{:.4},{:.4},{:.4},\
{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},\
{:.4},{:.1},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.4},{:.4},\
{:.4},{:.4},{:.4},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{},{},{},\
{},{},{},{},{},{}",
            x.t, x.x, x.y, x.z, x.speed, x.yaw, x.yaw_rate, x.roll, x.pitch, x.wing_amp, x.mode,
            x.pow_l, x.pow_r, x.steer_l, x.steer_r, x.walk_l, x.walk_r, x.land_l, x.land_r, x.mn9,
            x.dn02, x.dn07, x.sapp, x.land_dn, x.dn_filt, x.odor_l, x.odor_r, x.flow_l, x.flow_r,
            x.loom, x.taste, x.wall_dist,
            x.tau_aero[0], x.tau_aero[1], x.tau_aero[2],
            x.tau_damp[0], x.tau_damp[1], x.tau_damp[2],
            x.wroll, x.wpitch, x.wyaw, x.tilt_l, x.tilt_r, x.tilt_deg,
            x.fz_body, x.fz_world, x.steer_l_hz, x.steer_r_hz,
            x.touch[0] as u8, x.touch[1] as u8, x.touch[2] as u8,
            x.tau_yaw_tilt, x.tau_yaw_amp,
            x.wall_hits, x.takeoffs, x.landings, x.eats,
            x.win_spikes, x.tot_spikes
        )?;
    }
    f.flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Report

const TEMPLATE: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<title>FlyVerse behaviour report</title>
<style>
:root{--bg:#0d1117;--panel:#161b22;--line:#30363d;--fg:#e6edf3;--dim:#8b949e;--acc:#58a6ff}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--fg);font:13px/1.5 ui-sans-serif,system-ui,-apple-system,Segoe UI,Roboto,sans-serif;padding:20px 24px}
h1{font-size:19px;margin:0 0 2px}h2{font-size:13px;text-transform:uppercase;letter-spacing:.08em;color:var(--dim);margin:26px 0 10px;font-weight:600}
.sub{color:var(--dim);margin:0 0 18px}
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:12px}
.card{background:var(--panel);border:1px solid var(--line);border-radius:10px;padding:12px 14px}
.card h3{margin:0 0 8px;font-size:12px;color:var(--dim);font-weight:600;letter-spacing:.04em;text-transform:uppercase}
.kv{display:flex;justify-content:space-between;gap:10px;padding:2px 0;border-bottom:1px dotted #21262d}
.kv:last-child{border-bottom:0}.kv b{font-variant-numeric:tabular-nums;font-weight:600}
.warn{color:#f0883e}.good{color:#3fb950}.bad{color:#f85149}
canvas{width:100%;display:block;border-radius:6px;background:#0b0f14}
.note{background:#161b22;border-left:3px solid var(--acc);padding:10px 14px;border-radius:6px;color:var(--dim);margin:10px 0}
table{border-collapse:collapse;width:100%;font-size:12px}
th,td{text-align:left;padding:6px 8px;border-bottom:1px solid var(--line);vertical-align:top}
th{color:var(--dim);font-weight:600}
code{background:#0b0f14;padding:1px 5px;border-radius:4px;font-size:11px;color:#79c0ff}
</style></head><body>
<h1>FlyVerse behaviour report</h1>
<p class="sub" id="sub"></p>

<h2>Headline</h2>
<div class="grid" id="head"></div>

<h2>Where it went</h2>
<div class="grid">
  <div class="card"><h3>Floor occupancy, airborne (darker = more time)</h3><canvas id="occ" height="300"></canvas></div>
  <div class="card"><h3>Altitude distribution, airborne</h3><canvas id="alt" height="300"></canvas></div>
</div>

<h2>What it did over time</h2>
<div class="grid">
  <div class="card" style="grid-column:1/-1"><h3>Mode timeline</h3><canvas id="mode" height="70"></canvas></div>
  <div class="card" style="grid-column:1/-1"><h3>Altitude and speed</h3><canvas id="ts" height="220"></canvas></div>
  <div class="card" style="grid-column:1/-1"><h3>Wall proximity and the loom channel (does a wall ahead move the steering?)</h3><canvas id="loom" height="200"></canvas></div>
</div>

<h2>The loom test</h2>
<div class="note" id="loomnote"></div>

<h2>Provenance: what is real, what is surrogate</h2>
<table id="prov">
<tr><th>Layer</th><th>Source</th><th>Detail</th></tr>
<tr><td>Connectome graph</td><td class="good">real</td><td>Janelia FlyEM <b>MaleCNS v1.0</b>, 166,700 neurons / 24,469,412 edges, CC BY 4.0. Loaded from the official pack, no pruning or sampling.</td></tr>
<tr><td>Neuron dynamics</td><td class="good">real</td><td>LIF at <code>DT_MS = 0.1</code> ms, dense vote + ring propagation, <code>src/lif.rs</code>.</td></tr>
<tr><td>Spike rates</td><td class="good">real (rate-coded)</td><td>Motor and descending read-outs are measured from actual spikes in the network, not assigned.</td></tr>
<tr><td>Stimulus: vnc_sensory</td><td class="warn">reference replay</td><td>6,370 body IDs driven at 150 Hz from the dataset's own reference set. Fixed Poisson, <b>not</b> generated by the simulated body.</td></tr>
<tr><td>Odour field</td><td class="warn">surrogate</td><td>Finite-core exponential plume with an upwind virtual source, <code>room.rs::odor</code>. A stimulus field, not a fluid solve.</td></tr>
<tr><td>Optic flow</td><td class="warn">surrogate</td><td>Translation + rotation proxy from speed and yaw rate, <code>sim.rs::sense</code>. Not a rendered optic array.</td></tr>
<tr><td>Looming wall</td><td class="warn">surrogate</td><td>Time-to-contact with the nearest wall face, mapped to 0..1 and driven at up to 60 Hz. This is the only wall-proximity signal the brain gets.</td></tr>
<tr><td>Wingbeat</td><td class="warn">surrogate</td><td>Real wingbeat is ~200 Hz; the visual phase runs at 19 Hz so the stroke is visible. Aerodynamics are a tuned quadratic lift/drag model, <code>body.rs</code>.</td></tr>
<tr><td>Attitude and altitude</td><td class="warn">surrogate</td><td>Stabilisation, altitude targeting, banking and heading are engineered control loops. The dataset has no muscles, no VNC and no donor body state.</td></tr>
<tr><td>Gait and collision</td><td class="warn">surrogate</td><td>Walk speed cap, wall bounce restitution 0.15, takeoff/landing timers. No leg kinematics from data.</td></tr>
</table>

<script>
const S = __DATA__;
const SUM = __SUMMARY__;
const MODES = ['GROUND','TAKEOFF','CRUISE','LANDING','FEEDING'];
const MCOL = ['#484f58','#3fb950','#58a6ff','#f0883e','#a371f7'];

function kv(k,v,cls){return `<div class="kv"><span>${k}</span><b class="${cls||''}">${v}</b></div>`}
const n=(x,d=1)=>(x===null||x===undefined||!isFinite(x))?'-':(+x).toFixed(d);

document.getElementById('sub').textContent =
  `seed ${SUM.config.seed} - ${n(SUM.run.sim_seconds,1)} s simulated - ${SUM.config.samples} samples - ` +
  `${SUM.neural.neurons} neurons / ${SUM.neural.edges} edges - ${n(SUM.run.steps_per_second,0)} steps/s - ` +
  `${n(SUM.run.realtime_factor,3)}x realtime`;

const g=SUM.mode_percent, e=SUM.events, m=SUM.movement, a=SUM.altitude_mm, tl=SUM.turning, lr=SUM.loom_response;
document.getElementById('head').innerHTML =
  `<div class="card"><h3>Flight budget</h3>`+
  kv('ground', n(g.GROUND)+' %')+kv('takeoff',n(g.TAKEOFF)+' %')+kv('cruise',n(g.CRUISE)+' %')+
  kv('landing',n(g.LANDING)+' %')+kv('feeding',n(g.FEEDING)+' %')+`</div>`+
  `<div class="card"><h3>Events</h3>`+
  kv('takeoffs',e.takeoffs)+kv('landings',e.landings)+
  kv('wall hits', e.wall_hits, e.wall_hits_per_min>20?'bad':'warn')+
  kv('wall hits / min', n(e.wall_hits_per_min))+
  kv('one every', (e.mean_seconds_between_wall_hits==null || !isFinite(e.mean_seconds_between_wall_hits))?'never':n(e.mean_seconds_between_wall_hits)+' s')+
  kv('meals',e.eats)+`</div>`+
  `<div class="card"><h3>Movement</h3>`+
  kv('path', n(m.path_horizontal_mm,0)+' mm')+kv('net displacement', n(m.net_horizontal_displacement_mm,0)+' mm')+
  kv('tortuosity', n(m.tortuosity,2))+kv('mean speed', n(m.mean_speed_mm_s,0)+' mm/s')+
  kv('p95 speed', n(m.p95_speed_mm_s,0)+' mm/s')+kv('max speed', n(m.max_speed_mm_s,0)+' mm/s')+`</div>`+
  `<div class="card"><h3>Altitude and walls</h3>`+
  kv('alt mean', n(a.mean,1)+' mm')+kv('alt p05 / p95', n(a.p05,1)+' / '+n(a.p95,1))+
  kv('alt max', n(a.max,1)+' mm')+kv('ceiling contacts', n(a.ceiling_contacts_pct_of_airborne,1)+' % of airborne')+
  kv('within 20 mm of a wall', n(SUM.walls.pct_airborne_within_20mm,1)+' %', SUM.walls.pct_airborne_within_20mm>50?'bad':'')+`</div>`+
  `<div class="card"><h3>Steering</h3>`+
  kv('mean |yaw rate|', n(tl.mean_abs_yaw_rate_rad_s,2)+' rad/s')+
  kv('turning > 0.5', n(tl.pct_airborne_turning_gt_0p5,1)+' %')+
  kv('full circles', n(tl.full_circles_airborne,2))+
  kv('net yaw change', n(tl.net_yaw_change_rad,2)+' rad')+
  kv('mean |steer diff|', n(tl.steer_differential_mean_abs,3))+`</div>`+
  `<div class="card"><h3>Neural</h3>`+
  kv('spikes total', SUM.neural.total_spikes.toLocaleString())+
  kv('spikes / sim s', n(SUM.neural.spikes_per_sim_second,0))+
  kv('mean rate', n(SUM.neural.mean_rate_hz,2)+' Hz')+
  kv('active neurons', SUM.neural.active_neurons.toLocaleString())+
  kv('stimulus targets', SUM.neural.stimulus_targets)+`</div>`;

document.getElementById('loomnote').innerHTML =
  `<b>corr(loom, steer differential) = ${n(lr.pearson_loom_vs_steer_differential,3)}</b>, ` +
  `corr(loom, yaw rate) = ${n(lr.pearson_loom_vs_yaw_rate,3)}. ` +
  `With a wall filling the view (loom &gt; 0.5) mean |yaw rate| is ${n(lr.mean_abs_yaw_rate_when_loom_gt_0p5,2)} rad/s ` +
  `versus ${n(lr.mean_abs_yaw_rate_when_loom_lt_0p2,2)} when the way is clear; mean |steer| ` +
  `${n(lr.mean_abs_steer_when_loom_gt_0p5,3)} versus ${n(lr.mean_abs_steer_when_loom_lt_0p2,3)}. ` +
  `${lr.samples_imminent_wall.toLocaleString()} of the airborne samples had a wall imminent. ` +
  (Math.abs(lr.pearson_loom_vs_steer_differential) < 0.1
    ? `<span class="bad">A correlation near zero means the looming signal is not reaching the wings: the steering motor neurons are not being driven by it, so nothing steers the fly away from a wall.</span>`
    : `<span class="good">A non-zero correlation means the looming signal does move the steering motor neurons.</span>`);

function fit(id, h){
  const c=document.getElementById(id), r=window.devicePixelRatio||1;
  c.width=c.clientWidth*r; c.height=(h||c.clientHeight)*r;
  const x=c.getContext('2d'); x.setTransform(r,0,0,r,0,0);
  return [x, c.clientWidth, h||c.clientHeight];
}

// Occupancy heatmap
(function(){
  const [x,W,H]=fit('occ',300);
  const g=SUM.occupancy_airborne, ny=g.length, nx=g[0].length;
  const cw=W/nx, ch=H/ny, mx=Math.max(...g.flat(),1e-9);
  for(let r=0;r<ny;r++)for(let c=0;c<nx;c++){
    const v=g[r][c]/mx, al=Math.pow(v,0.45);
    x.fillStyle=`rgba(88,166,255,${(al*0.92).toFixed(3)})`;
    x.fillRect(c*cw, H-(r+1)*ch, cw-1, ch-1);
  }
  x.strokeStyle='#30363d'; x.strokeRect(0.5,0.5,W-1,H-1);
  x.fillStyle='#8b949e'; x.font='10px ui-monospace,monospace';
  x.fillText('x __ROOMX__ mm', 6, H-6); x.fillText('y __ROOMY__ mm  (top-down)', 6, 12);
})();

// Altitude histogram
(function(){
  const [x,W,H]=fit('alt',300);
  const h=SUM.altitude_mm.hist_10mm, mx=Math.max(...h,1e-9), bw=W/h.length;
  for(let i=0;i<h.length;i++){
    const bh=(h[i]/mx)*(H-30);
    x.fillStyle= i*10<40 ? '#3fb950' : '#58a6ff';
    x.fillRect(i*bw+1, H-20-bh, bw-2, bh);
  }
  x.fillStyle='#8b949e'; x.font='10px ui-monospace,monospace';
  x.fillText('0', 2, H-6); x.fillText('220 mm', W-46, H-6);
  x.fillText('table top 40 mm', 2, 12);
  x.strokeStyle='#30363d'; x.beginPath(); x.moveTo(0,H-20); x.lineTo(W,H-20); x.stroke();
})();

// Mode timeline
(function(){
  const [x,W,H]=fit('mode',70);
  const n=S.length;
  for(let i=0;i<n;i++){
    x.fillStyle=MCOL[S[i].mode];
    x.fillRect(i/n*W, 18, Math.max(1, W/n), 26);
  }
  x.font='10px ui-monospace,monospace';
  let lx=0;
  MODES.forEach((m,i)=>{ x.fillStyle=MCOL[i]; x.fillRect(lx,4,9,9);
    x.fillStyle='#8b949e'; x.fillText(m, lx+12, 12); lx+=12+x.measureText(m).width+14; });
  x.fillStyle='#8b949e'; x.fillText('t = 0 s', 2, H-3); x.fillText('t = '+n(S[S.length-1].t,1)+' s', W-58, H-3);
})();

// Altitude + speed
(function(){
  const [x,W,H]=fit('ts',220);
  const pad=26, n=S.length;
  const zmax=Math.max(...S.map(s=>s.z))*1.08||1;
  const vmax=Math.max(...S.map(s=>s.speed))*1.08||1;
  const px=i=>pad+ (i/(n-1||1))*(W-pad-8);
  const py=(v,mx)=>H-18-(v/mx)*(H-34);
  x.strokeStyle='#21262d'; x.beginPath(); x.moveTo(pad,H-18); x.lineTo(W-8,H-18); x.stroke();
  x.strokeStyle='#58a6ff'; x.lineWidth=1.4; x.beginPath();
  S.forEach((s,i)=>{ const X=px(i),Y=py(s.z,zmax); i?x.lineTo(X,Y):x.moveTo(X,Y); }); x.stroke();
  x.strokeStyle='#f0883e'; x.beginPath();
  S.forEach((s,i)=>{ const X=px(i),Y=py(s.speed,vmax); i?x.lineTo(X,Y):x.moveTo(X,Y); }); x.stroke();
  x.font='10px ui-monospace,monospace';
  x.fillStyle='#58a6ff'; x.fillText('altitude (max '+n(zmax,0)+' mm)', pad+4, 12);
  x.fillStyle='#f0883e'; x.fillText('speed (max '+n(vmax,0)+' mm/s)', pad+150, 12);
  x.fillStyle='#8b949e'; x.fillText('t = 0', pad-14, H-4); x.fillText(n(S[S.length-1].t,0)+' s', W-30, H-4);
})();

// Loom + steering
(function(){
  const [x,W,H]=fit('loom',200);
  const pad=26, n=S.length;
  const px=i=>pad+(i/(n-1||1))*(W-pad-8);
  const py=v=>H-18-v*(H-34);
  x.strokeStyle='#21262d'; x.beginPath(); x.moveTo(pad,H-18); x.lineTo(W-8,H-18); x.stroke();
  x.strokeStyle='#f85149'; x.globalAlpha=0.85; x.lineWidth=1.1; x.beginPath();
  S.forEach((s,i)=>{ const X=px(i),Y=py(Math.min(1,s.loom)); i?x.lineTo(X,Y):x.moveTo(X,Y); }); x.stroke();
  x.globalAlpha=1;
  const sd=S.map(s=>Math.abs(s.steer_r-s.steer_l));
  const sdmax=Math.max(...sd,1e-6);
  x.strokeStyle='#3fb950'; x.lineWidth=1.4; x.beginPath();
  S.forEach((s,i)=>{ const X=px(i),Y=py(Math.abs(s.steer_r-s.steer_l)/sdmax); i?x.lineTo(X,Y):x.moveTo(X,Y); }); x.stroke();
  x.font='10px ui-monospace,monospace';
  x.fillStyle='#f85149'; x.fillText('loom 0..1 (wall ahead)', pad+4, 12);
  x.fillStyle='#3fb950'; x.fillText('|steer differential| (scaled)', pad+160, 12);
  x.fillStyle='#8b949e'; x.fillText('t = 0', pad-14, H-4);
})();
</script></body></html>
"#;

pub(crate) fn write_report(path: &Path, samples: &[Sample], summary: &serde_json::Value) -> Result<()> {
    // Keep the embedded series bounded; the CSV carries the full rate.
    let stride = (samples.len() / 1500).max(1);
    let slim: Vec<&Sample> = samples.iter().step_by(stride).collect();
    let data = serde_json::to_string(&slim)?;
    // The floor-plan axes are labelled from the arena actually simulated, so a
    // scaled run (FLYVERSE_ROOM_SCALE) does not label its map with the shipped
    // bounds.
    let bound = |axis: &str, i: usize| -> f64 {
        summary["config"]["arena_mm"][axis][i]
            .as_f64()
            .unwrap_or(f64::NAN)
    };
    let html = TEMPLATE
        .replace("__DATA__", &data)
        .replace("__SUMMARY__", &serde_json::to_string(summary)?)
        .replace("__ROOMX__", &format!("{:.0}..{:.0}", bound("x", 0), bound("x", 1)))
        .replace("__ROOMY__", &format!("{:.0}..{:.0}", bound("y", 0), bound("y", 1)));
    std::fs::write(path, html)?;
    Ok(())
}