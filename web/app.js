// FlyVerse frontend: live virtual room + real fly body + connectome activity panel.
// Plain ES modules, no bundler, no frameworks, no external network beyond same-origin.
//
// Coordinate system: millimetres, Z up, origin on the floor at the room centre.
//   room: x [-300,300]  y [-220,220]  z [0,220]
//   fly local frame: +X forward (headward), +Y left, +Z up (matches the room frame)
// Cameras use camera.up = (0,0,1) for the Z-up world (top-down mode switches to +Y up).
//
// NOTE ON UNITS: rig.json offsets are millimetres (fly body length ~2.3 mm, thorax
// 2.1 mm above the foot plane). The STL meshes are stored in METRES (the wing spans
// 0.0023 units = 2.3 mm), so mesh geometry is multiplied by MESH_UNIT_TO_MM to land
// in the same mm frame as the rig and the room. No arbitrary scaling is applied.

import * as THREE from 'three';
import { STLLoader } from './vendor/STLLoader.js';
import { buildCuteFly, applyLegScale, FLIGHT_POSE } from './fly_cute.js';

const MESH_UNIT_TO_MM = 1000.0;
const API = '/api';

// Build stamp: proves which revision a given browser tab is actually running, so a stale
// cached module can never be confused with a live bug. This runs at module evaluation,
// before boot(), so it still reports if the render loop later blocks.
const APP_VER = '20260912f';
window.__fvBooted = true;
(function stampBuild() {
  const el = document.getElementById('f-appver');
  if (el) el.textContent = 'app: ' + APP_VER;
})();

const ROOM = {
  x: [-300, 300],
  y: [-220, 220],
  z: [0, 220],
  size: [600, 440, 220],
  centre: new THREE.Vector3(0, 0, 110),
};

// ---------------------------------------------------------------- tiny helpers
const $ = (id) => document.getElementById(id);

function ff(v, digits) {
  if (v === null || v === undefined || (typeof v === 'number' && !isFinite(v))) return '-';
  return Number(v).toFixed(digits === undefined ? 2 : digits);
}
function int(v) {
  if (v === null || v === undefined || (typeof v === 'number' && !isFinite(v))) return '-';
  return Math.round(v).toLocaleString('en-US');
}
function setText(id, s) {
  const el = $(id);
  if (el && el.textContent !== s) el.textContent = s;
}
function clamp(v, a, b) { return v < a ? a : (v > b ? b : v); }

function makeBarRow(parent, label, cls) {
  const row = document.createElement('div');
  row.className = 'bar-row';
  const lab = document.createElement('span');
  lab.className = 'lab';
  lab.textContent = label;
  const bar = document.createElement('span');
  bar.className = 'bar ' + (cls || '');
  const fill = document.createElement('i');
  bar.appendChild(fill);
  const num = document.createElement('span');
  num.className = 'num';
  num.textContent = '-';
  row.appendChild(lab); row.appendChild(bar); row.appendChild(num);
  parent.appendChild(row);
  return { bar: fill, num: num };
}

// ---------------------------------------------------------------- constants
const REGION_LABELS = [
  'sensory_leg', 'olfaction', 'taste', 'visual_motion', 'visual_loom',
  'flight_descent', 'flight_motor', 'walking_motor', 'landing_motor',
  'feeding_motor', 'central', 'everything_else',
];
const MOTOR_LABELS = [
  ['flight_power_l', 'fl_power_l'], ['flight_power_r', 'fl_power_r'],
  ['flight_steer_l', 'fl_steer_l'], ['flight_steer_r', 'fl_steer_r'],
  ['walk_l', 'walk_l'], ['walk_r', 'walk_r'],
  ['land_l', 'land_l'], ['land_r', 'land_r'],
  ['mn9', 'mn9 (feeding)'],
];
const SENSORY_LABELS = [
  ['odor_l', 'odor L'], ['odor_r', 'odor R'],
  ['retina_l', 'retina L'], ['retina_r', 'retina R'],
];
const LEG_IDS = ['lf', 'lm', 'lh', 'rf', 'rm', 'rh'];
const TRIPOD_A = { lf: 1, rh: 1, lm: 1 };

// ---------------------------------------------------------------- DOM: build panels
const regionUI = [], motorUI = [], sensoryUI = [];
(function buildPanels() {
  const r = $('regions');
  REGION_LABELS.forEach(function (lab, i) {
    regionUI.push(makeBarRow(r, i + ' ' + lab, 'b-hot'));
  });
  const m = $('motors');
  MOTOR_LABELS.forEach(function (kv) {
    motorUI.push(makeBarRow(m, kv[1], 'b-motor'));
  });
  const s = $('sensory');
  SENSORY_LABELS.forEach(function (kv) {
    sensoryUI.push(makeBarRow(s, kv[1], 'b-green'));
  });
})();

function updateBar(ui, v, scale) {
  if (!ui) return;
  if (v === null || v === undefined || !isFinite(v)) {
    ui.bar.style.width = '0%';
    ui.num.textContent = '-';
    return;
  }
  const pct = clamp(v * (scale === undefined ? 100 : scale), 0, 100);
  ui.bar.style.width = pct.toFixed(1) + '%';
  ui.num.textContent = ff(v, 2);
}

// ---------------------------------------------------------------- three.js core
const viewCanvas = $('view');
const LITE = new URLSearchParams(location.search).has('lite');
const renderer = new THREE.WebGLRenderer({ canvas: viewCanvas, antialias: !LITE });
renderer.setPixelRatio(LITE ? 0.5 : Math.min(window.devicePixelRatio || 1, 2));
renderer.setClearColor(0x05070a, 1);

const scene = new THREE.Scene();
scene.background = new THREE.Color(0x05070a);
scene.fog = new THREE.Fog(0x05070a, 900, 2600);

const camera = new THREE.PerspectiveCamera(42, 1, 0.6, 6000);

function sizeView() {
  const w = viewCanvas.clientWidth || window.innerWidth;
  const h = viewCanvas.clientHeight || window.innerHeight;
  renderer.setSize(w, h, false);
  camera.aspect = w / Math.max(1, h);
  camera.updateProjectionMatrix();
}

// ---------------------------------------------------------------- lighting
// A warm studio mood rather than the old black void. The previous setup was a
// near-black background lit by a cold blue key, which made the pale model read
// like a specimen in a freezer.
function gradientBackground(stops) {
  const cv = document.createElement('canvas');
  cv.width = 4; cv.height = 256;
  const g = cv.getContext('2d');
  const grd = g.createLinearGradient(0, 0, 0, 256);
  for (const [at, col] of stops) grd.addColorStop(at, col);
  g.fillStyle = grd; g.fillRect(0, 0, 4, 256);
  const t = new THREE.CanvasTexture(cv);
  if (THREE.SRGBColorSpace) t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

scene.background = gradientBackground([[0, '#212a3d'], [0.55, '#161c29'], [1, '#0b0e15']]);
scene.fog = new THREE.Fog(0x161c29, 1100, 3000);

renderer.toneMapping = THREE.ACESFilmicToneMapping;
renderer.toneMappingExposure = 1.05;

const ambient = new THREE.AmbientLight(0xfff0e2, 0.35);
scene.add(ambient);
const hemi = new THREE.HemisphereLight(0xd6e6ff, 0x3a2410, 1.0);
scene.add(hemi);

const keyLight = new THREE.DirectionalLight(0xfff2dc, 1.35);
keyLight.position.set(-260, -180, 420);
scene.add(keyLight);

const fillLight = new THREE.DirectionalLight(0xa8c8ff, 0.55);
fillLight.position.set(300, 260, 120);
scene.add(fillLight);

// Warm rim from behind: separates the body from the floor and adds the glow
// that makes the cartoon body look alive.
const rimLight = new THREE.DirectionalLight(0xffd9a0, 0.7);
rimLight.position.set(140, 330, -280);
scene.add(rimLight);

// a soft light travelling with the fly so it reads against the walls
const flyLight = new THREE.PointLight(0xffe6c4, 1100, 200, 2);
scene.add(flyLight);

// ---------------------------------------------------------------- room
const roomGroup = new THREE.Group();
scene.add(roomGroup);

const woodTex = (function () {
  const loader = new THREE.TextureLoader();
  const t = loader.load('./assets/oak-v1.png',
    function (tex) {
      tex.wrapS = tex.wrapT = THREE.RepeatWrapping;
      tex.colorSpace = THREE.SRGBColorSpace;
      // 1, not 4: anisotropic filtering is a software-rasteriser cliff. On
      // llvmpipe/SwiftShader the multi-tap aniso path on a full-screen floor
      // stalls the first frame for minutes.
      tex.anisotropy = 1;
      tex.needsUpdate = true;
    },
    undefined,
    function () { /* texture missing: plain colour floor still renders */ });
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
})();

function woodMaterial(repeatX, repeatY, base) {
  const tex = woodTex.clone();
  tex.needsUpdate = true;
  tex.wrapS = tex.wrapT = THREE.RepeatWrapping;
  tex.repeat.set(repeatX, repeatY);
  tex.colorSpace = THREE.SRGBColorSpace;
  // Lambert, not Standard: this scene is also opened on software GL / VMs where
  // the full PBR program costs minutes to compile on first frame. Lambert keeps
  // the textured look and renders immediately everywhere.
  return new THREE.MeshLambertMaterial({
    map: tex, color: base === undefined ? 0xb99b74 : base,
    side: THREE.DoubleSide,
  });
}

// floor
const floor = new THREE.Mesh(new THREE.PlaneGeometry(ROOM.size[0], ROOM.size[1]), woodMaterial(4, 3));
floor.position.set(0, 0, 0);
roomGroup.add(floor);

// ceiling (semi transparent so an outside camera can see in)
const ceiling = new THREE.Mesh(
  new THREE.PlaneGeometry(ROOM.size[0], ROOM.size[1]),
  new THREE.MeshLambertMaterial({ color: 0x9fb4c4, transparent: true, opacity: 0.10, side: THREE.DoubleSide, depthWrite: false })
);
ceiling.position.set(0, 0, ROOM.z[1]);
roomGroup.add(ceiling);

// walls
function wall(w, h, mat) {
  return new THREE.Mesh(new THREE.PlaneGeometry(w, h), mat);
}
function wallMaterial() {
  return new THREE.MeshLambertMaterial({
    color: 0x74b9d8,
    transparent: true, opacity: 0.11, side: THREE.DoubleSide, depthWrite: false,
  });
}
const wX = wall(ROOM.size[1], ROOM.size[2], wallMaterial());
wX.rotation.y = Math.PI / 2;
wX.position.set(ROOM.x[1], 0, ROOM.z[1] / 2);
roomGroup.add(wX);

const wX2 = wall(ROOM.size[1], ROOM.size[2], wallMaterial());
wX2.rotation.y = -Math.PI / 2;
wX2.position.set(ROOM.x[0], 0, ROOM.z[1] / 2);
roomGroup.add(wX2);

const wY = wall(ROOM.size[0], ROOM.size[2], wallMaterial());
wY.rotation.x = -Math.PI / 2;
wY.position.set(0, ROOM.y[1], ROOM.z[1] / 2);
roomGroup.add(wY);

const wY2 = wall(ROOM.size[0], ROOM.size[2], wallMaterial());
wY2.rotation.x = Math.PI / 2;
wY2.position.set(0, ROOM.y[0], ROOM.z[1] / 2);
roomGroup.add(wY2);

// Crisp room frame. The glassy walls are only 11% opaque, which is nearly invisible
// against a dark background, so the fly read as floating in empty space instead of
// flying inside a room. These opaque edge lines make the box legible without hiding
// the fly the way solid walls would.
const roomEdges = new THREE.LineSegments(
  new THREE.EdgesGeometry(new THREE.BoxGeometry(ROOM.size[0], ROOM.size[1], ROOM.size[2])),
  new THREE.LineBasicMaterial({ color: 0x6fc4ea, transparent: true, opacity: 0.5 })
);
roomEdges.position.copy(ROOM.centre);
roomGroup.add(roomEdges);

// grid on the floor so motion reads (600 x 440, 40 mm cells)
(function floorGrid() {
  const pts = [];
  const z = 0.6;
  for (let x = ROOM.x[0]; x <= ROOM.x[1] + 0.01; x += 40) {
    pts.push(x, ROOM.y[0], z, x, ROOM.y[1], z);
  }
  for (let y = ROOM.y[0]; y <= ROOM.y[1] + 0.01; y += 40) {
    pts.push(ROOM.x[0], y, z, ROOM.x[1], y, z);
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(pts, 3));
  const m = new THREE.LineBasicMaterial({ color: 0x2a4a5e, transparent: true, opacity: 0.55 });
  roomGroup.add(new THREE.LineSegments(g, m));
})();

// table: top surface at z=40, footprint x 40..280, y -160..120, 12 mm thick top
const TABLE = { top: 40, thickness: 12, x: [40, 280], y: [-160, 120] };
(function buildTable() {
  const cx = (TABLE.x[0] + TABLE.x[1]) / 2, cy = (TABLE.y[0] + TABLE.y[1]) / 2;
  const w = TABLE.x[1] - TABLE.x[0], d = TABLE.y[1] - TABLE.y[0];
  const top = new THREE.Mesh(new THREE.BoxGeometry(w, d, TABLE.thickness), woodMaterial(3, 3, 0xc0a27c));
  top.position.set(cx, cy, TABLE.top - TABLE.thickness / 2);
  roomGroup.add(top);

  const legMat = new THREE.MeshLambertMaterial({ color: 0x6b4f34 });
  const legH = TABLE.top - TABLE.thickness;
  const ins = 18;
  const legX = [TABLE.x[0] + ins, TABLE.x[1] - ins];
  const legY = [TABLE.y[0] + ins, TABLE.y[1] - ins];
  for (let i = 0; i < 2; i++) {
    for (let j = 0; j < 2; j++) {
      const leg = new THREE.Mesh(new THREE.BoxGeometry(14, 14, legH), legMat);
      leg.position.set(legX[i], legY[j], legH / 2);
      roomGroup.add(leg);
    }
  }
})();

// sugar cube (food) - 8x8x8 mm, sits on the table top, slightly emissive
const sugar = new THREE.Mesh(
  new THREE.BoxGeometry(8, 8, 8),
  new THREE.MeshLambertMaterial({
    color: 0xfff4d6, emissive: 0xffb347, emissiveIntensity: 0.45,
  })
);
sugar.position.set(160, 40, 44);
roomGroup.add(sugar);
const sugarHalo = new THREE.Mesh(
  new THREE.RingGeometry(6, 7.4, 32),
  new THREE.MeshBasicMaterial({ color: 0xffc35e, transparent: true, opacity: 0.5, side: THREE.DoubleSide, depthWrite: false })
);
roomGroup.add(sugarHalo);

// fly ground marker + vertical beacon (makes the fly findable in wide views)
const flyRing = new THREE.Mesh(
  new THREE.RingGeometry(6.4, 7.6, 40),
  new THREE.MeshBasicMaterial({ color: 0x3ad0ff, transparent: true, opacity: 0.55, side: THREE.DoubleSide, depthWrite: false })
);
flyRing.position.set(0, 0, 0.7);
roomGroup.add(flyRing);

const beaconGeo = new THREE.BufferGeometry();
beaconGeo.setAttribute('position', new THREE.Float32BufferAttribute([0, 0, 0, 0, 0, 60], 3));
const beacon = new THREE.Line(beaconGeo, new THREE.LineBasicMaterial({ color: 0x3ad0ff, transparent: true, opacity: 0.28 }));
roomGroup.add(beacon);

// flight trail
const TRAIL_MAX = 700;
const trail = new THREE.Line(
  new THREE.BufferGeometry().setAttribute('position', new THREE.Float32BufferAttribute(new Float32Array(TRAIL_MAX * 3), 3)),
  new THREE.LineBasicMaterial({ color: 0x39d0ff, transparent: true, opacity: 0.5 })
);
trail.frustumCulled = false;
// Hidden until updateTrail() has at least two points. Submitting a THREE.Line
// with fewer than two vertices hangs a software rasteriser (SwiftShader) even
// with an empty draw range, so the trail must not be in the draw list yet.
trail.geometry.setDrawRange(0, 0);
trail.visible = false;
scene.add(trail);
const trailPts = [];
const trailAttr = trail.geometry.getAttribute('position');

// ---------------------------------------------------------------- fly rig
const flyRoot = new THREE.Group();          // world transform from the stream
flyRoot.position.set(0, 0, 60);
scene.add(flyRoot);

const flyRig = new THREE.Group();           // rig hierarchy root (child = c_thorax)
flyRoot.add(flyRig);

const bodyObjects = {};                     // short name -> Object3D
const anim = { wings: [], halteres: [], abdomen: [], legs: {}, loaded: false };

const bodyMat = new THREE.MeshPhongMaterial({ color: 0x9fb2bd, shininess: 42, specular: 0x6b7c88, flatShading: true });
const wingMat = new THREE.MeshPhongMaterial({ color: 0xcfe4f0, shininess: 90, specular: 0xa9c6d8, transparent: true, opacity: 0.55, side: THREE.DoubleSide, depthWrite: false });
const eyeMat = new THREE.MeshPhongMaterial({ color: 0x5c2a24, shininess: 60, specular: 0x8a4a3c, flatShading: true });

function materialFor(short) {
  if (short.indexOf('wing') >= 0) return wingMat;
  if (short.indexOf('eye') >= 0) return eyeMat;
  return bodyMat;
}

const geomCache = {};                        // 'file.stl' -> BufferGeometry (mm)
const loader = new STLLoader();

async function fetchGeometry(file) {
  if (geomCache[file]) return geomCache[file];
  const res = await fetch('./assets/meshes/' + file);
  if (!res.ok) throw new Error('mesh ' + file + ' http ' + res.status);
  const buf = await res.arrayBuffer();
  const g = loader.parse(buf);
  g.scale(MESH_UNIT_TO_MM, MESH_UNIT_TO_MM, MESH_UNIT_TO_MM);
  g.computeVertexNormals();
  g.computeBoundingSphere();
  geomCache[file] = g;
  return g;
}

// Right-hand bodies have no mesh in the rig; mirror the matching left mesh across Y
// (the rig carries no rotations, so a Y-mirror of position + geometry is exact).
function mirrorGeometry(src) {
  const g = src.clone();
  const p = g.getAttribute('position');
  const n = g.getAttribute('normal');
  const pa = p.array, na = n.array;
  for (let i = 0; i < pa.length; i += 3) pa[i + 1] = -pa[i + 1];
  for (let i = 0; i < na.length; i += 3) na[i + 1] = -na[i + 1];
  // reflection flips handedness: reverse each triangle's winding to keep normals outward
  for (let t = 0; t < pa.length; t += 9) {
    for (let k = 0; k < 3; k++) {
      const ap = t + k * 3, bp = t + (2 - k) * 3;
      if (k < 2) {
        let tmp = pa[ap]; pa[ap] = pa[bp]; pa[bp] = tmp;
        tmp = pa[ap + 1]; pa[ap + 1] = pa[bp + 1]; pa[bp + 1] = tmp;
        tmp = pa[ap + 2]; pa[ap + 2] = pa[bp + 2]; pa[bp + 2] = tmp;
      }
      let t2 = na[ap]; na[ap] = na[bp]; na[bp] = t2;
      t2 = na[ap + 1]; na[ap + 1] = na[bp + 1]; na[bp + 1] = t2;
      t2 = na[ap + 2]; na[ap + 2] = na[bp + 2]; na[bp + 2] = t2;
    }
  }
  g.computeBoundingSphere();
  return g;
}

function mirroredFileFor(short) {
  // r_wing -> l_wing, rf_coxa -> lf_coxa, r_eye -> l_eye ...
  if (short.charAt(0) !== 'r') return null;
  const alt = 'l' + short.slice(1);
  return alt + '.stl';                       // caller checks existence in the rig
}

async function buildRig() {
  console.log('[flyverse] rig: begin');
  const res = await fetch('./assets/rig.json');
  if (!res.ok) throw new Error('rig.json http ' + res.status);
  const rig = await res.json();
  const bodies = rig.bodies || [];
  console.log('[flyverse] rig: bodies=' + bodies.length);
  const byShort = {};
  bodies.forEach(function (b) { byShort[b.short] = b; });

  // 1. Object3D per body, parented per the rig
  bodies.forEach(function (b) {
    const o = new THREE.Object3D();
    o.name = b.name;
    o.position.set(b.pos[0], b.pos[1], b.pos[2]);
    bodyObjects[b.short] = o;
    o.userData.short = b.short;
  });
  bodies.forEach(function (b) {
    const o = bodyObjects[b.short];
    const parent = b.parent ? bodyObjects[b.parent.replace('fly/', '')] : null;
    if (parent) parent.add(o); else flyRig.add(o);
  });

  // 2. visual body. Default is the stylised cartoon fly (web/fly_cute.js);
  //    ?model=anatomical swaps the NeuroMechFly meshes back in.
  const MODEL = (new URLSearchParams(location.search).get('model') || 'cute').toLowerCase();
  let cute = null;
  if (MODEL !== 'anatomical') {
    // Shorten the leg chains before the meshes are built, so the stubby
    // cartoon segments line up with the joints.
    const shortened = applyLegScale(bodyObjects);
    cute = buildCuteFly(THREE, { detailed: !LITE });
    let attached = 0;
    bodies.forEach(function (b) {
      const p = cute.parts[b.short];
      if (p && bodyObjects[b.short]) { bodyObjects[b.short].add(p); attached++; }
    });
    console.log('[flyverse] model=cute parts=' + attached + '/' + bodies.length +
      ' geos=' + cute.stats.geometries + ' legsShortened=' + shortened);
    if (attached === 0) {
      console.warn('[flyverse] cute model matched no rig nodes, using anatomical meshes');
      cute = null;
    }
    document.documentElement.dataset.model = cute ? 'cute' : 'anatomical';
  } else {
    document.documentElement.dataset.model = 'anatomical';
  }

  // 3. meshes
  const jobs = [];
  bodies.forEach(function (b) {
    const short = b.short;
    if (cute) return;                       // cartoon body already covers this node
    let file = b.mesh ? b.mesh : null;
    let mirror = false;
    if (!file) {
      const mf = mirroredFileFor(short);
      if (mf && byShort['l' + short.slice(1)] && byShort['l' + short.slice(1)].mesh) {
        file = byShort['l' + short.slice(1)].mesh;
        mirror = true;
      }
    }
    if (!file) return;
    jobs.push((async function () {
      try {
        let g = await fetchGeometry(file);
        if (mirror) g = mirrorGeometry(g);
        const mesh = new THREE.Mesh(g, materialFor(short));
        mesh.frustumCulled = false;
        bodyObjects[short].add(mesh);
      } catch (e) {
        console.warn('[flyverse] mesh failed:', file, e && e.message);
      }
    })());
  });
  console.log('[flyverse] rig: jobs=' + jobs.length + ' awaiting');
  await Promise.all(jobs);
  console.log('[flyverse] rig: jobs done, meshes=' + flyRig.children.length);

  // 3. animation handles
  anim.wings = [bodyObjects['l_wing'], bodyObjects['r_wing']].filter(Boolean);
  anim.halteres = [bodyObjects['l_haltere'], bodyObjects['r_haltere']].filter(Boolean);
  anim.abdomen = ['c_abdomen12', 'c_abdomen3', 'c_abdomen4', 'c_abdomen5', 'c_abdomen6']
    .map(function (s) { return bodyObjects[s]; }).filter(Boolean);
  LEG_IDS.forEach(function (id) {
    const seg = {};
    ['coxa', 'trochanterfemur', 'tibia', 'tarsus1', 'tarsus2', 'tarsus3', 'tarsus4', 'tarsus5'].forEach(function (s) {
      const o = bodyObjects[id + '_' + s];
      if (o) seg[s] = o;
    });
    anim.legs[id] = { side: id.charAt(0) === 'l' ? 1 : -1, tripod: TRIPOD_A[id] ? 1 : 0, seg: seg };
  });
  anim.loaded = true;
}

// ---------------------------------------------------------------- pose animation
const pose = {
  pos: new THREE.Vector3(0, 0, 60),
  quat: new THREE.Quaternion(),
};

function updatePose(t, dt, f) {
  const b = (f && f.body) || null;

  // wings / halteres
  const amp = b && typeof b.wing_amp === 'number' ? clamp(b.wing_amp, 0, 1) : (b && b.airborne ? 0.7 : 0);
  const flapHz = 6 + 9 * amp;
  poseState.wingPhase += dt * Math.PI * 2 * flapHz;
  if (b && typeof b.wing_phase === 'number' && isFinite(b.wing_phase)) {
    // gently resync toward the server phase (server beats at ~200 Hz, we draw it slowed)
    let d = b.wing_phase - poseState.wingPhase;
    d = Math.atan2(Math.sin(d), Math.cos(d));
    poseState.wingPhase += d * 0.12;
  }
  const ph = poseState.wingPhase;
  const swing = Math.sin(ph);
  const fold = 1 - clamp(amp * 3.2, 0, 1);          // wings folded back when grounded
  for (let i = 0; i < anim.wings.length; i++) {
    const w = anim.wings[i];
    const s = w.name.indexOf('/l_') > 0 ? 1 : -1;
    w.rotation.x = s * amp * 1.05 * swing;
    w.rotation.z = s * (fold * 1.32 + amp * 0.16 * Math.cos(ph));
    w.rotation.y = s * amp * 0.10 * Math.cos(ph);
  }
  for (let i = 0; i < anim.halteres.length; i++) {
    const h = anim.halteres[i];
    const s = h.name.indexOf('/l_') > 0 ? 1 : -1;
    h.rotation.x = s * amp * 0.55 * Math.sin(ph + Math.PI);
  }

  // legs: tripod gait when grounded, tucked when airborne
  const grounded = !!(b && b.airborne === false);
  const gait = b && typeof b.gait_phase === 'number' && isFinite(b.gait_phase) ? b.gait_phase : 0;
  const walkness = b ? clamp((b.speed || 0) / 40 + (grounded ? 0.55 : 0), 0, 1) : 0;
  LEG_IDS.forEach(function (id) {
    const leg = anim.legs[id];
    if (!leg) return;
    const tripodPhase = gait + (leg.tripod ? 0 : Math.PI);
    const sw = Math.sin(tripodPhase);
    const lift = Math.max(0, Math.sin(tripodPhase + Math.PI / 2));
    const targetSwing = -sw * 0.26 * walkness;
    const targetLiftX = leg.side * lift * 0.12 * walkness;
    const tuck = grounded ? 0 : 1;
    const seg = leg.seg;
    if (seg.coxa) {
      seg.coxa.rotation.y = targetSwing;
      seg.coxa.rotation.x = targetLiftX + tuck * leg.side * FLIGHT_POSE.coxa;
    }
    if (seg.trochanterfemur) {
      seg.trochanterfemur.rotation.x = leg.side * (lift * 0.38 - 0.06) * walkness + tuck * leg.side * FLIGHT_POSE.femur;
      seg.trochanterfemur.rotation.y = -sw * 0.12 * walkness;
    }
    if (seg.tibia) seg.tibia.rotation.x = leg.side * (0.30 - lift * 0.34) * walkness + tuck * leg.side * FLIGHT_POSE.tibia;
    ['tarsus1', 'tarsus2', 'tarsus3', 'tarsus4', 'tarsus5'].forEach(function (k, i) {
      if (!seg[k]) return;
      const curl = grounded ? (0.10 - lift * 0.16) * (1 - i * 0.08) : FLIGHT_POSE.tarsus;
      seg[k].rotation.x = leg.side * curl * (grounded ? walkness : 1);
      seg[k].rotation.y = grounded ? sw * 0.05 : 0;
    });
  });

  // abdomen: gentle bend from climb/descent intent and mode
  const alt = f && f.neural && typeof f.neural.alt_drive === 'number' ? f.neural.alt_drive : 0;
  const bend = clamp(alt, -1, 1) * 0.16;
  for (let i = 0; i < anim.abdomen.length; i++) {
    const a = anim.abdomen[i];
    a.rotation.y = bend * (0.5 + i * 0.28);
    a.rotation.x = Math.sin(t * 1.7 + i * 0.6) * 0.012 * (0.4 + amp);
  }
}

const poseState = { wingPhase: 0 };

// ---------------------------------------------------------------- brain panel
const brain = {
  ready: false,
  count: 0,
  bright: null,
  attr: null,
  lit: 0,
  renderer: null,
  scene: null,
  camera: null,
  points: null,
  spin: null,
};

function initBrainRenderer() {
  const canvas = $('brain');
  const wrap = $('brain-wrap');
  const w = wrap.clientWidth || 310;
  const h = wrap.clientHeight || 310;
  const r = new THREE.WebGLRenderer({ canvas: canvas, antialias: !LITE });
  r.setPixelRatio(1);
  r.setSize(w, h, false);
  r.setClearColor(0x020408, 1);
  const s = new THREE.Scene();
  const cam = new THREE.PerspectiveCamera(45, w / h, 0.01, 100);
  cam.position.set(1.85, -1.35, 1.5);
  cam.up.set(0, 0, 1);
  cam.lookAt(0, 0, 0);
  brain.renderer = r;
  brain.scene = s;
  brain.camera = cam;

  const box = new THREE.LineSegments(
    new THREE.EdgesGeometry(new THREE.BoxGeometry(2.15, 2.15, 2.15)),
    new THREE.LineBasicMaterial({ color: 0x1d3d50, transparent: true, opacity: 0.7 })
  );
  s.add(box);
  brain.spin = new THREE.Group();
  s.add(brain.spin);
}

const BRAIN_VERT = [
  'attribute float bright;',
  'uniform float uSize;',
  'varying float vB;',
  'void main() {',
  '  vB = bright;',
  '  vec4 mv = modelViewMatrix * vec4(position, 1.0);',
  '  // uSize is in cloud units (~0.02 = about 1.5 px at the default camera);',
  '  // the sprite is clamped so a single soma can never blow up the fill rate.',
  '  float px = uSize * (0.45 + 1.55 * bright) * (260.0 / max(0.15, -mv.z));',
  '  gl_PointSize = clamp(px, 1.0, 6.0);',
  '  gl_Position = projectionMatrix * mv;',
  '}',
].join('\n');

const BRAIN_FRAG = [
  'varying float vB;',
  'void main() {',
  '  vec2 d = gl_PointCoord - vec2(0.5);',
  '  float r2 = dot(d, d);',
  '  if (r2 > 0.25) discard;',
  '  float a = smoothstep(0.25, 0.02, r2);',
  '  float t = clamp(vB, 0.0, 1.0);',
  '  vec3 c = mix(vec3(0.10, 0.16, 0.42), vec3(0.16, 0.52, 1.0), smoothstep(0.01, 0.30, t));',
  '  c = mix(c, vec3(1.0, 0.82, 0.20), smoothstep(0.30, 0.70, t));',
  '  c = mix(c, vec3(1.0, 1.0, 0.95), smoothstep(0.70, 0.98, t));',
  '  gl_FragColor = vec4(c, a * (0.16 + 0.84 * t));',
  '}',
].join('\n');

async function loadBrainPositionsOnce() {
  const ac = new AbortController();
  const timer = setTimeout(() => ac.abort(), 15000);
  const res = await fetch(API + '/brain/positions', { signal: ac.signal });
  if (!res.ok) { clearTimeout(timer); throw new Error('http ' + res.status); }
  const buf = await res.arrayBuffer();
  clearTimeout(timer);
  if (buf.byteLength < 12 || (buf.byteLength % 12) !== 0) throw new Error('bad payload ' + buf.byteLength + ' bytes');
  const n = buf.byteLength / 12;
  const src = new Float32Array(buf);

  let cx = 0, cy = 0, cz = 0;
  let mnx = 1e30, mny = 1e30, mnz = 1e30, mxx = -1e30, mxy = -1e30, mxz = -1e30;
  for (let i = 0; i < n; i++) {
    const x = src[i * 3], y = src[i * 3 + 1], z = src[i * 3 + 2];
    cx += x; cy += y; cz += z;
    if (x < mnx) mnx = x; if (x > mxx) mxx = x;
    if (y < mny) mny = y; if (y > mxy) mxy = y;
    if (z < mnz) mnz = z; if (z > mxz) mxz = z;
  }
  cx /= n; cy /= n; cz /= n;
  let rad = 0;
  for (let i = 0; i < n; i++) {
    const dx = src[i * 3] - cx, dy = src[i * 3 + 1] - cy, dz = src[i * 3 + 2] - cz;
    const d = Math.sqrt(dx * dx + dy * dy + dz * dz);
    if (d > rad) rad = d;
  }
  if (!(rad > 0)) rad = 1;
  const k = 1.0 / rad;                       // normalise the cloud to radius 1

  const pos = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) {
    pos[i * 3] = (src[i * 3] - cx) * k;
    pos[i * 3 + 1] = (src[i * 3 + 1] - cy) * k;
    pos[i * 3 + 2] = (src[i * 3 + 2] - cz) * k;
  }

  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.BufferAttribute(pos, 3));
  const bright = new Float32Array(n);
  const ba = new THREE.BufferAttribute(bright, 1);
  ba.setUsage(THREE.DynamicDrawUsage);
  g.setAttribute('bright', ba);
  g.computeBoundingSphere();

  const mat = new THREE.ShaderMaterial({
    uniforms: { uSize: { value: 0.022 } },
    vertexShader: BRAIN_VERT,
    fragmentShader: BRAIN_FRAG,
    transparent: true,
    blending: THREE.AdditiveBlending,
    depthWrite: false,
  });
  const points = new THREE.Points(g, mat);
  brain.spin.add(points);

  brain.points = points;
  brain.bright = bright;
  brain.attr = ba;
  brain.count = n;
  brain.ready = true;

  const el = $('brain-msg');
  if (el) el.classList.add('hidden');
  console.log('[flyverse] brain cloud loaded: ' + n + ' somata, radius ' + rad.toFixed(3));
}

// Retry wrapper: a silent hang here left the panel on "loading positions..." forever,
// with no clue why. Each attempt is now bounded, failures retry, and the panel reports
// the actual reason so the failure is visible instead of looking like a stuck load.
async function loadBrainPositions() {
  const msg = $('brain-msg');
  let lastErr = null;
  for (let attempt = 1; attempt <= 3; attempt++) {
    try {
      await loadBrainPositionsOnce();
      return;
    } catch (e) {
      lastErr = e;
      const why = (e && e.name === 'AbortError') ? 'timed out after 15s' : ((e && e.message) || String(e));
      console.warn('[flyverse] positions attempt ' + attempt + '/3 failed: ' + why);
      if (msg) msg.textContent = 'positions failed (' + attempt + '/3): ' + why;
      if (attempt < 3) await new Promise((r) => setTimeout(r, attempt * 1000));
    }
  }
  if (msg) msg.textContent = 'positions FAILED: ' + ((lastErr && lastErr.message) || lastErr);
  throw lastErr;
}

function addSpikeIndices(u32) {
  if (!brain.ready) return;
  const n = brain.count, b = brain.bright;
  const len = u32.length;
  for (let i = 0; i < len; i++) {
    const idx = u32[i];
    if (idx < n) b[idx] += 1.0;
  }
  brain.attr.needsUpdate = true;
}

function decayAndUploadBrain(factor) {
  if (!brain.ready) return;
  const b = brain.bright;
  let lit = 0;
  for (let i = 0; i < b.length; i++) {
    const v = b[i] * factor;
    b[i] = v < 0.004 ? 0 : v;
    if (v > 0.08) lit++;
  }
  brain.lit = lit;
  brain.attr.needsUpdate = true;
}

// ---------------------------------------------------------------- data plumbing
const state = {
  lastFrame: null,
  lastFrameWall: 0,
  lastFramePrevWall: 0,
  frames: 0,
  link: 'connecting',
  spikesFrom: 0,
  spikesPolled: 0,
  spikesBusy: false,
  spikesFails: 0,
  spikesTimer: null,
  health: null,
};

function setLink(txt, cls) {
  const el = $('v-link');
  if (!el) return;
  el.textContent = txt;
  el.className = 'v ' + (cls || '');
}

function applyFrame(f, wallNow) {
  state.lastFrame = f;
  if (state.lastFrameWall) state.lastFramePrevWall = state.lastFrameWall;
  state.lastFrameWall = wallNow;
  state.frames++;
  setLink('live', 'ok');

  const b = f.body || {};
  const pos = Array.isArray(b.pos) ? b.pos : [0, 0, 0];
  const q = Array.isArray(b.quat) && b.quat.length === 4 ? b.quat : [1, 0, 0, 0];
  pose.pos.set(pos[0] || 0, pos[1] || 0, pos[2] || 0);
  pose.quat.set(q[1] || 0, q[2] || 0, q[3] || 0, q[0] === undefined ? 1 : q[0]);

  if (f.food && Array.isArray(f.food.pos)) {
    sugar.position.set(f.food.pos[0], f.food.pos[1], f.food.pos[2]);
    sugarHalo.position.set(f.food.pos[0], f.food.pos[1], 0.8);
  }

  // HUD
  setText('v-stim', f.stimulus === undefined ? '-' : String(f.stimulus));
  setText('v-simtime', ff((f.sim_ms || 0) / 1000, 2) + ' s');
  setText('v-rt', ff(f.wall_rt, 2) + 'x');
  setText('v-seq', int(f.seq));
  setText('v-paused', f.paused ? 'yes' : 'no');
  const btn = $('btn-pause');
  if (btn) btn.textContent = f.paused ? 'resume' : 'pause';

  setText('v-mode', b.mode === undefined ? '-' : String(b.mode));
  setText('v-air', b.airborne ? 'yes' : 'no');
  setText('v-speed', ff(b.speed, 1) + ' mm/s');
  setText('v-alt', ff(pos[2], 1) + ' mm');
  setText('v-pos', ff(pos[0], 1) + ', ' + ff(pos[1], 1) + ', ' + ff(pos[2], 1));
  setText('v-pry', ff(b.pitch, 2) + ' / ' + ff(b.roll, 2) + ' / ' + ff(b.yaw, 2));
  setText('v-fooddist', f.food && f.food.dist !== undefined ? ff(f.food.dist, 1) + ' mm' : '-');
  setText('v-wingamp', ff(b.wing_amp, 2));
  setText('v-legs', int(b.legs_supported));

  const nn = f.neural || {};
  setText('v-sps', ff(spikesPerSecond(), 0) + ' /s');
  setText('v-spwin', int(nn.spikes_window));
  setText('v-sptot', int(nn.spikes_total));
  setText('v-active', int(nn.active_neurons));
  setText('v-meanrate', ff(nn.mean_rate_hz, 2) + ' Hz');
  setText('v-tkdrive', ff(nn.takeoff_drive, 3));
  setText('v-altdrive', ff(nn.alt_drive, 3));
  setText('v-hall', ff(nn.hall, 3));

  const ev = f.events || {};
  setText('v-eats', int(ev.eats));
  setText('v-takeoffs', int(ev.takeoffs));
  setText('v-landings', int(ev.landings));
  setText('v-wallhits', int(ev.wall_hits));

  if (Array.isArray(nn.regions)) {
    for (let i = 0; i < regionUI.length; i++) updateBar(regionUI[i], nn.regions[i]);
  }
  const mo = f.motors || {};
  updateMotorSensory(mo, nn);
}

function updateMotorSensory(mo, nn) {
  for (let i = 0; i < MOTOR_LABELS.length; i++) updateBar(motorUI[i], mo[MOTOR_LABELS[i][0]]);
  for (let i = 0; i < SENSORY_LABELS.length; i++) updateBar(sensoryUI[i], nn[SENSORY_LABELS[i][0]]);
}

function spikesPerSecond() {
  const f = state.lastFrame;
  if (!f || !f.neural) return null;
  const dt = state.lastFrameWall - state.lastFramePrevWall;
  if (!(dt > 0.0005)) return null;
  return (f.neural.spikes_window || 0) / dt;
}

// ---------------------------------------------------------------- SSE
let sse = null;
function connectStream() {
  if (typeof EventSource === 'undefined') { setLink('no EventSource', 'bad'); return; }
  try {
    sse = new EventSource(API + '/stream');
  } catch (e) {
    setLink('stream error', 'bad');
    return;
  }
  sse.addEventListener('hello', function () {
    fetchHealth();
  });
  sse.addEventListener('frame', function (ev) {
    let f = null;
    try { f = JSON.parse(ev.data); } catch (e) { console.warn('[flyverse] bad frame json'); return; }
    applyFrame(f, performance.now() / 1000);
  });
  sse.onopen = function () { setLink('connected', 'ok'); };
  sse.onerror = function () {
    // EventSource reconnects on its own
    if (performance.now() / 1000 - state.lastFrameWall > 2.5) setLink('connecting', 'warn');
  };
}

async function fetchHealth() {
  try {
    const res = await fetch(API + '/health');
    if (!res.ok) return;
    const h = await res.json();
    state.health = h;
    if (h.neurons) setText('f-neurons', Number(h.neurons).toLocaleString('en-US'));
    if (h.edges) setText('f-edges', Number(h.edges).toLocaleString('en-US'));
  } catch (e) { /* health is optional */ }
}

// ---------------------------------------------------------------- spike polling
function base64ToU32(b64) {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  const n = bytes.length >>> 2;                 // whole u32s only
  const buf = new ArrayBuffer(n * 4);           // fresh, 4-byte aligned
  new Uint8Array(buf).set(bytes.subarray(0, n * 4));
  return new Uint32Array(buf);
}

async function pollSpikes() {
  if (state.spikesBusy) return;
  state.spikesBusy = true;
  try {
    const res = await fetch(API + '/spikes?from=' + state.spikesFrom);
    if (!res.ok) throw new Error('http ' + res.status);
    const j = await res.json();
    if (j && typeof j.to === 'number') state.spikesFrom = j.to;
    if (j && typeof j.dt === 'string' && j.dt.length) {
      const u = base64ToU32(j.dt);
      state.spikesPolled += u.length;
      addSpikeIndices(u);
    }
    state.spikesFails = 0;
  } catch (e) {
    state.spikesFails++;
    if (state.spikesFails > 6) stopSpikePolling();
  } finally {
    state.spikesBusy = false;
  }
}

function startSpikePolling() {
  if (state.spikesTimer) return;
  // 200 ms, not 120: the payload is capped server-side (SPIKE_MAX) and the
  // decode is JS-bound, so a faster poll only starves the render loop.
  state.spikesTimer = setInterval(pollSpikes, 200);
  pollSpikes();
}

function stopSpikePolling() {
  if (state.spikesTimer) { clearInterval(state.spikesTimer); state.spikesTimer = null; }
}

// ---------------------------------------------------------------- cameras
const CAMS = { CHASE: 0, ROOM: 1, TOP: 2, ORBIT: 3 };
const camState = {
  mode: CAMS.CHASE,
  chaseDist: 24,
  orbit: { radius: 70, theta: 2.2, phi: 1.05 },
  drag: false,
  lastX: 0,
  lastY: 0,
};

const CAM_MARGIN = 9;
const ROOM_CAM_POS = new THREE.Vector3(470, -400, 330);   // outside the room, walls are see-through
function clampInsideRoom(v) {
  v.x = clamp(v.x, ROOM.x[0] + CAM_MARGIN, ROOM.x[1] - CAM_MARGIN);
  v.y = clamp(v.y, ROOM.y[0] + CAM_MARGIN, ROOM.y[1] - CAM_MARGIN);
  v.z = clamp(v.z, ROOM.z[0] + 4, ROOM.z[1] - 4);
  return v;
}

const _fwd = new THREE.Vector3(), _left = new THREE.Vector3(), _up = new THREE.Vector3();
const _desired = new THREE.Vector3(), _look = new THREE.Vector3();

function updateCamera(dt) {
  const flyPos = flyRoot.position;
  if (camState.mode === CAMS.CHASE) {
    camera.up.set(0, 0, 1);
    _fwd.set(1, 0, 0).applyQuaternion(flyRoot.quaternion);
    _left.set(0, 1, 0).applyQuaternion(flyRoot.quaternion);
    _up.set(0, 0, 1).applyQuaternion(flyRoot.quaternion);
    const D = camState.chaseDist;
    _desired.copy(flyPos)
      .addScaledVector(_fwd, -D)
      .addScaledVector(_left, D * 0.42)
      .addScaledVector(_up, D * 0.30);
    clampInsideRoom(_desired);
    camera.position.lerp(_desired, 1 - Math.exp(-dt / 0.05));
    _look.copy(flyPos).addScaledVector(_fwd, 3.0);
    camera.lookAt(_look);
  } else if (camState.mode === CAMS.ROOM) {
    // outside the room, looking in through the semi transparent walls
    camera.up.set(0, 0, 1);
    camera.position.lerp(ROOM_CAM_POS, 1 - Math.exp(-dt / 0.15));
    camera.lookAt(20, -10, 70);
  } else if (camState.mode === CAMS.TOP) {
    camera.up.set(0, 1, 0);
    camera.position.lerp(new THREE.Vector3(flyPos.x, flyPos.y, 400), 1 - Math.exp(-dt / 0.12));
    camera.lookAt(flyPos.x, flyPos.y, flyPos.z);
  } else {
    camera.up.set(0, 0, 1);
    const o = camState.orbit;
    const r = o.radius;
    _desired.set(
      flyPos.x + r * Math.cos(o.phi) * Math.cos(o.theta),
      flyPos.y + r * Math.cos(o.phi) * Math.sin(o.theta),
      flyPos.z + r * Math.sin(o.phi)
    );
    clampInsideRoom(_desired);
    camera.position.lerp(_desired, 1 - Math.exp(-dt / 0.06));
    camera.lookAt(flyPos);
  }
}

function setCamMode(m) {
  camState.mode = m;
  for (let i = 0; i < 4; i++) {
    const b = $('cam-' + i);
    if (b) b.className = 'cam' + (i === m ? ' on' : '');
  }
  if (m === CAMS.ORBIT) {
    // seed the orbit from the current camera position so the switch is not a jump
    const d = new THREE.Vector3().subVectors(camera.position, flyRoot.position);
    camState.orbit.radius = clamp(d.length(), 12, 320);
    camState.orbit.theta = Math.atan2(d.y, d.x);
    camState.orbit.phi = Math.asin(clamp(d.z / Math.max(1e-3, d.length()), -1, 1));
  }
}

// ---------------------------------------------------------------- input
(function wireInput() {
  window.addEventListener('resize', function () { sizeView(); });
  window.addEventListener('keydown', function (e) {
    if (e.key === '0') setCamMode(CAMS.CHASE);
    else if (e.key === '1') setCamMode(CAMS.ROOM);
    else if (e.key === '2') setCamMode(CAMS.TOP);
    else if (e.key === '3') setCamMode(CAMS.ORBIT);
    else if (e.key === ' ') { e.preventDefault(); sendControl('toggle_pause'); }
    else if (e.key === 'r' || e.key === 'R') sendControl('reset');
    else if (e.key === '+' || e.key === '=') camState.chaseDist = clamp(camState.chaseDist * 0.8, 4, 900);
    else if (e.key === '-') camState.chaseDist = clamp(camState.chaseDist * 1.25, 4, 900);
  });

  const cv = viewCanvas;
  cv.addEventListener('pointerdown', function (e) {
    if (camState.mode !== CAMS.ORBIT) setCamMode(CAMS.ORBIT);
    camState.drag = true; camState.lastX = e.clientX; camState.lastY = e.clientY;
    cv.setPointerCapture(e.pointerId);
  });
  cv.addEventListener('pointermove', function (e) {
    if (!camState.drag) return;
    const dx = e.clientX - camState.lastX, dy = e.clientY - camState.lastY;
    camState.lastX = e.clientX; camState.lastY = e.clientY;
    camState.orbit.theta -= dx * 0.006;
    camState.orbit.phi = clamp(camState.orbit.phi + dy * 0.005, -1.35, 1.45);
    camState.orbit.radius = clamp(camState.orbit.radius, 12, 340);
  });
  cv.addEventListener('pointerup', function (e) {
    camState.drag = false;
    try { cv.releasePointerCapture(e.pointerId); } catch (err) { /* ignore */ }
  });
  cv.addEventListener('wheel', function (e) {
    e.preventDefault();
    if (camState.mode === CAMS.CHASE) camState.chaseDist = clamp(camState.chaseDist * Math.exp(e.deltaY * 0.0012), 4, 900);
    else if (camState.mode === CAMS.ORBIT) camState.orbit.radius = clamp(camState.orbit.radius * Math.exp(e.deltaY * 0.0012), 12, 340);
  }, { passive: false });

  for (let i = 0; i < 4; i++) {
    const b = $('cam-' + i);
    if (b) b.addEventListener('click', function () { setCamMode(i); });
  }
  const bp = $('btn-pause');
  if (bp) bp.addEventListener('click', function () { sendControl('toggle_pause'); });
  const br = $('btn-reset');
  if (br) br.addEventListener('click', function () { sendControl('reset'); });
})();

async function sendControl(cmd) {
  try {
    const res = await fetch(API + '/control?cmd=' + encodeURIComponent(cmd));
    if (!res.ok) console.warn('[flyverse] control ' + cmd + ' -> http ' + res.status);
  } catch (e) {
    console.warn('[flyverse] control ' + cmd + ' failed: ' + (e && e.message));
  }
}

// ---------------------------------------------------------------- loop
const clock = new THREE.Clock();
let frameNo = 0;
const perf = { frames: 0, fps: 0, mark: performance.now() / 1000 };

function updateTrail() {
  const p = flyRoot.position;
  const last = trailPts.length ? trailPts[trailPts.length - 1] : null;
  if (!last || last.distanceToSquared(p) > 1.0) {
    trailPts.push(p.clone());
    while (trailPts.length > TRAIL_MAX) trailPts.shift();
    for (let i = 0; i < TRAIL_MAX; i++) {
      const src = trailPts[i] || trailPts[trailPts.length - 1] || p;
      trailAttr.array[i * 3] = src.x;
      trailAttr.array[i * 3 + 1] = src.y;
      trailAttr.array[i * 3 + 2] = src.z;
    }
    trailAttr.needsUpdate = true;
    trail.geometry.setDrawRange(0, trailPts.length);
    trail.visible = !SKIP.has('trail') && trailPts.length >= 2;
  }
}

function updateMarkers(t) {
  const p = flyRoot.position;
  flyRing.position.set(p.x, p.y, 0.8);
  flyRing.rotation.z = t * 0.6;
  const bp = beaconGeo.getAttribute('position');
  bp.array[0] = p.x; bp.array[1] = p.y; bp.array[2] = 0.8;
  bp.array[3] = p.x; bp.array[4] = p.y; bp.array[5] = p.z;
  bp.needsUpdate = true;
  flyLight.position.set(p.x, p.y, Math.min(p.z + 6, ROOM.z[1] - 4));
  sugarHalo.position.set(sugar.position.x, sugar.position.y, 0.8);
  sugarHalo.scale.setScalar(1 + 0.12 * Math.sin(t * 2.4));
  sugar.rotation.z = Math.sin(t * 0.7) * 0.02;
}

function tick() {
  const dt = Math.min(clock.getDelta(), 0.1);
  const t = clock.elapsedTime;
  frameNo++;
  // QA isolation: on the first debug frame, render each top-level object alone
  // so a single pathological material/geometry identifies itself.
  if (ISO && frameNo === 1) {
    const kids = scene.children.slice();
    const vis = kids.map((o) => o.visible);
    for (const ch of kids) {
      if (ch.isLight) continue;
      kids.forEach((o) => { o.visible = (o === ch) || !!o.isLight; });
      console.log('[flyverse] isolate ' + ch.type + ' name=' + (ch.name || '-') +
        ' kids=' + (ch.children ? ch.children.length : 0));
      const a = performance.now();
      renderer.render(scene, camera);
      console.log('[flyverse]   -> ' + (performance.now() - a).toFixed(0) + ' ms');
    }
    kids.forEach((o, i) => { o.visible = vis[i]; });
    console.log('[flyverse] isolation done');
  }


  // render-rate meter (QA + HUD sanity)
  perf.frames++;
  const now = performance.now() / 1000;
  if (now - perf.mark >= 1) {
    perf.fps = perf.frames / (now - perf.mark);
    perf.frames = 0;
    perf.mark = now;
  }

  // smooth toward the latest stream pose
  const k = 1 - Math.exp(-dt / 0.055);
  const far = flyRoot.position.distanceToSquared(pose.pos) > 40000;   // >200 mm: snap
  if (far) {
    flyRoot.position.copy(pose.pos);
    flyRoot.quaternion.copy(pose.quat);
  } else {
    flyRoot.position.lerp(pose.pos, k);
    flyRoot.quaternion.slerp(pose.quat, k);
  }

  updatePose(t, dt, state.lastFrame);
  updateCamera(dt);
  updateTrail();
  if (DBG && frameNo <= 5) console.log('[flyverse] tick #' + frameNo + ' pose+cam+trail ok');
  updateMarkers(t);
  if (DBG && frameNo <= 5) console.log('[flyverse] tick #' + frameNo + ' markers ok');

  if (brain.ready) {
    decayAndUploadBrain(Math.pow(0.9, dt * 60));
    brain.spin.rotation.z += dt * 0.09;
    brain.spin.rotation.y += dt * 0.02;
    brain.renderer.render(brain.scene, brain.camera);
    setText('v-bpoll', int(state.spikesPolled));
    setText('v-blit', int(brain.lit));
  }

  // link state watchdog
  if (state.lastFrameWall && performance.now() / 1000 - state.lastFrameWall > 2.5) {
    setLink('stalled', 'bad');
  }

  const t0 = DBG ? performance.now() : 0;
  renderer.render(scene, camera);
  if (DBG) {
    renderMs += performance.now() - t0;
    renderN++;
    if (renderN === 1 || renderN % 5 === 0) {
      console.log('[flyverse] render avg ' + (renderMs / renderN).toFixed(1) + ' ms over ' +
        renderN + ' frames; drawCalls=' + renderer.info.render.calls +
        ' tris=' + renderer.info.render.triangles);
      renderMs = 0; renderN = 0;
    }
  }
}
let renderMs = 0, renderN = 0;

// ---------------------------------------------------------------- boot
// ?skip=brain,rig,spikes disables subsystems, for isolating a hang in QA.
const SKIP = new Set(
  (new URLSearchParams(location.search).get('skip') || '').split(',').filter(Boolean)
);
const DBG = new URLSearchParams(location.search).has('dbg');
const ISO = new URLSearchParams(location.search).has('iso');

async function boot() {
  sizeView();
  setLink('connecting', 'warn');
  renderer.setAnimationLoop(tick);

  connectStream();
  fetchHealth();
  if (!SKIP.has('spikes')) startSpikePolling();

  if (!SKIP.has('brain')) {
    try {
      initBrainRenderer();
      await loadBrainPositions();
    } catch (e) {
      console.warn('[flyverse] brain panel unavailable: ' + (e && e.message));
      // Keep the panel visible with the reason: hiding it made a real failure look
      // like a layout choice, and "loading positions..." looked like a stall forever.
      const m = $('brain-msg');
      if (m) {
        m.classList.remove('hidden');
        m.textContent = 'brain panel unavailable: ' + ((e && e.message) || e);
      }
    }
  }

  if (!SKIP.has('rig')) {
    try {
      await buildRig();
      console.log('[flyverse] fly rig built');
    } catch (e) {
      console.warn('[flyverse] rig failed to build: ' + (e && e.message));
    }
  }
  console.log('[flyverse] boot done (skip=' + [...SKIP].join(',') + ')');
}

// QA hook: lets an automated check read the live app state without a framework.
window.__flyverse = {
  state: state,
  brain: brain,
  anim: anim,
  perf: perf,
  flyRoot: flyRoot,
  flyRig: flyRig,
  camera: camera,
  renderer: renderer,
  camState: camState,
  pose: pose,
  meshCount: function () {
    let n = 0;
    flyRoot.traverse(function (o) { if (o.isMesh) n++; });
    return n;
  },
  hasBody: function (short) { return !!bodyObjects[short]; },
};

boot().catch(function (e) {
  console.warn('[flyverse] boot error: ' + (e && e.message));
});
