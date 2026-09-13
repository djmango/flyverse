// ---------------------------------------------------------------- fly_cute.js
// A stylised, friendly fly model built entirely from primitives.
//
// WHY THIS EXISTS
// The anatomical meshes (NeuroMechFly / FlyGym, Apache-2.0) are a faithful
// reconstruction of a real Drosophila: chalky, faceted, spindly legs. In the
// chase camera that reads as a cadaver cast. This module renders the SAME
// skeleton with a deliberately cartoonish body instead: plump proportions,
// oversized glossy eyes, banded abdomen, translucent iridescent wings.
//
// It is a visual layer only. It attaches one child object per rig node using
// the node names from assets/rig.json, so every existing animation path in
// app.js (wingbeat, haltere counter-swing, abdomen undulation, leg gait)
// drives it unchanged. Physics, telemetry and neuron counts are untouched.
//
// Scale is millimetres, matching the rig. A whole fly is about 2.3 mm long,
// so every dimension below is sub-millimetre by design.

// Local frame of each anatomical mesh in mm: s = bounding size, c = bounding
// centre. Generated from web/assets/meshes/*.stl by scripts/mesh_frames.cjs.
// Used here only as a size template, so the cartoon body occupies the same
// volume as the real one and the leg nodes stay anatomically placed.
export const STL_FRAMES = {
  c_abdomen12: { s: [0.473, 0.956, 0.724], c: [-0.242, 0, 0.014] },
  c_abdomen3: { s: [0.377, 0.98, 0.78], c: [-0.071, 0, -0.051] },
  c_abdomen4: { s: [0.345, 0.989, 0.721], c: [-0.091, 0, -0.061] },
  c_abdomen5: { s: [0.415, 0.898, 0.72], c: [-0.104, 0, -0.069] },
  c_abdomen6: { s: [0.58, 0.745, 0.63], c: [-0.161, 0, -0.045] },
  c_haustellum: { s: [0.355, 0.35, 0.243], c: [0.164, 0, -0.099] },
  c_head: { s: [0.511, 0.753, 0.678], c: [0.24, 0, 0.063] },
  c_rostrum: { s: [0.418, 0.414, 0.503], c: [-0.169, 0, -0.027] },
  c_thorax: { s: [1.064, 0.89, 1.079], c: [-0.492, 0, -0.096] },
  l_arista: { s: [0.064, 0.262, 0.137], c: [0.014, 0.117, 0.013] },
  l_eye: { s: [0.403, 0.216, 0.508], c: [-0.004, 0.307, -0.024] },
  l_funiculus: { s: [0.117, 0.123, 0.196], c: [0.008, -0.013, -0.103] },
  l_haltere: { s: [0.116, 0.37, 0.145], c: [-0.021, 0.171, 0.012] },
  l_pedicel: { s: [0.197, 0.184, 0.128], c: [0.079, 0.009, -0.017] },
  l_wing: { s: [1.084, 2.406, 0.435], c: [-0.137, 1.098, 0.154] },
  lf_coxa: { s: [0.152, 0.17, 0.46], c: [0.013, -0.002, -0.197] },
  lf_tarsus1: { s: [0.069, 0.06, 0.242], c: [0.004, -0.003, -0.14] },
  lf_tarsus2: { s: [0.059, 0.05, 0.157], c: [0.006, -0.003, -0.103] },
  lf_tarsus3: { s: [0.05, 0.046, 0.11], c: [0.008, -0.005, -0.073] },
  lf_tarsus4: { s: [0.043, 0.042, 0.097], c: [0.002, -0.005, -0.064] },
  lf_tarsus5: { s: [0.057, 0.054, 0.105], c: [0.001, -0.007, -0.065] },
  lf_tibia: { s: [0.109, 0.102, 0.55], c: [0.019, 0.006, -0.287] },
  lf_trochanterfemur: { s: [0.158, 0.138, 0.762], c: [-0.016, 0.021, -0.382] },
  lh_coxa: { s: [0.174, 0.161, 0.267], c: [-0.016, 0.03, -0.093] },
  lh_tarsus1: { s: [0.093, 0.082, 0.374], c: [-0.023, 0.014, -0.187] },
  lh_tarsus2: { s: [0.079, 0.064, 0.196], c: [-0.015, 0.014, -0.099] },
  lh_tarsus3: { s: [0.051, 0.046, 0.108], c: [-0.015, 0.012, -0.056] },
  lh_tarsus4: { s: [0.047, 0.045, 0.088], c: [-0.013, 0.011, -0.044] },
  lh_tarsus5: { s: [0.071, 0.066, 0.106], c: [-0.018, 0.013, -0.054] },
  lh_tibia: { s: [0.133, 0.116, 0.76], c: [0.006, -0.002, -0.349] },
  lh_trochanterfemur: { s: [0.17, 0.166, 0.874], c: [0.023, -0.009, -0.432] },
  lm_coxa: { s: [0.174, 0.202, 0.271], c: [-0.004, 0.017, -0.077] },
  lm_tarsus1: { s: [0.076, 0.061, 0.295], c: [-0.013, 0.01, -0.171] },
  lm_tarsus2: { s: [0.053, 0.048, 0.161], c: [-0.01, 0.01, -0.098] },
  lm_tarsus3: { s: [0.057, 0.047, 0.107], c: [-0.009, 0.011, -0.063] },
  lm_tarsus4: { s: [0.047, 0.04, 0.07], c: [-0.007, 0.01, -0.047] },
  lm_tarsus5: { s: [0.061, 0.062, 0.105], c: [-0.008, 0.011, -0.058] },
  lm_tibia: { s: [0.118, 0.097, 0.737], c: [0.011, 0.01, -0.358] },
  lm_trochanterfemur: { s: [0.147, 0.128, 0.836], c: [0.017, 0.008, -0.412] },
};

const SEGMENT_RE = /^([lr])([fmh])_/;

// How much shorter the cartoon legs are than the anatomical ones. Real
// Drosophila legs are about as long as the body, which in a close-up reads as a
// dangling spider. Shortening the chain to ~60% while keeping the inflated
// radius gives the stubby limbs a cartoon fly needs.
export const LEG_SEG_SCALE = 0.6;

// Shortens every leg by scaling the offsets BETWEEN its segments. The coxa
// stays anchored to the thorax, so each leg still starts where the anatomy puts
// it and the joints stay connected. Pair this with the matching mesh scaling in
// buildCuteFly or the segments will separate.
export function applyLegScale(nodes, k = LEG_SEG_SCALE) {
  const re = /^[lr][fmh]_(trochanterfemur|tibia|tarsus[1-5])$/;
  let n = 0;
  for (const short of Object.keys(nodes)) {
    if (re.test(short)) { nodes[short].position.multiplyScalar(k); n++; }
  }
  return n;
}

// Tucked-leg angles used while airborne, applied per leg with a sign that
// flips for the right side. Shared by the app (updatePose) and the preview so
// the two can never disagree. The negative femur and positive tibia make a
// Z-shaped fold, and the coxa angle splays each leg outward; a leg that hangs
// straight down in a bundle is what made the model read as a dead spider.
export const FLIGHT_POSE = { coxa: 0.55, femur: -0.35, tibia: 1.05, tarsus: -0.45 };

// Apply FLIGHT_POSE to a map of rig nodes (short name -> Object3D).
export function poseFlightLegs(nodes) {
  let n = 0;
  for (const short of Object.keys(nodes)) {
    if (!/^[lr][fmh]_/.test(short)) continue;
    const side = short.startsWith('r') ? -1 : 1;
    const o = nodes[short];
    if (short.endsWith('_coxa')) o.rotation.x = side * FLIGHT_POSE.coxa;
    else if (short.endsWith('_trochanterfemur')) o.rotation.x = side * FLIGHT_POSE.femur;
    else if (short.endsWith('_tibia')) o.rotation.x = side * FLIGHT_POSE.tibia;
    else if (/_tarsus\d$/.test(short)) o.rotation.x = side * FLIGHT_POSE.tarsus;
    else continue;
    n++;
  }
  return n;
}

// Which family each rig node belongs to.
function kindOf(short) {
  if (SEGMENT_RE.test(short)) return short.endsWith('coxa') ? 'coxaseg' : 'legseg';
  if (short === 'l_wing' || short === 'r_wing') return 'wing';
  if (short === 'l_eye' || short === 'r_eye') return 'eye';
  if (short.endsWith('_haltere')) return 'haltere';
  if (short.endsWith('_arista')) return 'arista';
  if (short.endsWith('_pedicel') || short.endsWith('_funiculus')) return 'antenna';
  if (short === 'c_thorax') return 'thorax';
  if (short === 'c_head') return 'head';
  if (short.startsWith('c_abdomen')) return 'abdomen';
  if (short === 'c_rostrum' || short === 'c_haustellum') return 'proboscis';
  return 'generic';
}

// The skeleton is bilateral: only the left copy of each mesh was measured, so
// mirror the Y offset for right-hand parts.
function frameFor(short) {
  let f = STL_FRAMES[short];
  let mirrored = false;
  if (!f) {
    const alt = 'l' + short.slice(1); // r_eye -> l_eye, r_wing -> l_wing
    if (STL_FRAMES[alt]) {
      f = STL_FRAMES[alt];
      mirrored = true;
    }
  }
  if (!f) return null;
  return { s: f.s, c: mirrored ? [f.c[0], -f.c[1], f.c[2]] : f.c };
}

export function buildCuteFly(THREE, opts = {}) {
  const detailed = opts.detailed !== false;
  const geos = [], mats = [], parts = {};
  const track = (g) => { geos.push(g); return g; };

  // -- materials ------------------------------------------------------------
  // Chitin: dark slate cuticle with a warm velvet sheen, so it reads soft
  // rather than glossy plastic.
  const chitin = new THREE.MeshPhysicalMaterial({
    color: 0x2b3040, roughness: 0.5, metalness: 0.0,
    clearcoat: 0.7, clearcoatRoughness: 0.35,
    sheen: 1.0, sheenColor: new THREE.Color(0xffd7a8), sheenRoughness: 0.8,
    iridescence: 0.22, iridescenceIOR: 1.5,
  });
  const chitinWarm = new THREE.MeshPhysicalMaterial({
    color: 0xd8762a, roughness: 0.45, metalness: 0.0,
    clearcoat: 0.8, clearcoatRoughness: 0.3,
    sheen: 0.8, sheenColor: new THREE.Color(0xffe0b0), sheenRoughness: 0.7,
    iridescence: 0.3, iridescenceIOR: 1.6,
  });
  const chitinPale = new THREE.MeshPhysicalMaterial({
    color: 0x4a4238, roughness: 0.55, metalness: 0.0,
    clearcoat: 0.55, clearcoatRoughness: 0.4,
    sheen: 0.7, sheenColor: new THREE.Color(0xffe6c8), sheenRoughness: 0.85,
  });
  // Compound eyes: oversized, near-black red with a hard wet highlight and
  // strong thin-film iridescence -- the single biggest cuteness lever.
  const eyeMat = new THREE.MeshPhysicalMaterial({
    color: 0x7d1f12, roughness: 0.05, metalness: 0.0,
    clearcoat: 1.0, clearcoatRoughness: 0.02,
    iridescence: 1.0, iridescenceIOR: 2.0,
    specularIntensity: 1.0,
    emissive: new THREE.Color(0x2a0805), emissiveIntensity: 1.0,
  });
  const legMat = new THREE.MeshPhysicalMaterial({
    color: 0x33313a, roughness: 0.55, metalness: 0.0,
    clearcoat: 0.5, clearcoatRoughness: 0.35,
    sheen: 0.5, sheenColor: new THREE.Color(0xffd7a8), sheenRoughness: 0.85,
  });
  const haltereMat = new THREE.MeshPhysicalMaterial({
    color: 0xdaa257, roughness: 0.28, metalness: 0.0, clearcoat: 1.0,
    clearcoatRoughness: 0.08, iridescence: 0.5, iridescenceIOR: 1.8,
  });
  const wingMat = new THREE.MeshPhysicalMaterial({
    color: 0xdce8ff, roughness: 0.1, metalness: 0.0,
    transparent: true, opacity: 0.55, side: THREE.DoubleSide,
    depthWrite: false, clearcoat: 1.0, clearcoatRoughness: 0.05,
    iridescence: 0.95, iridescenceIOR: 2.0,
    emissive: new THREE.Color(0x141c30), emissiveIntensity: 1.0,
  });
  if (detailed) wingMat.map = wingVeinTexture(THREE);
  mats.push(chitin, chitinWarm, chitinPale, eyeMat, legMat, haltereMat, wingMat);

  // -- helpers --------------------------------------------------------------
  // An ellipsoid sized to a mesh's bounding box, shrunk by k so the silhouette
  // is rounded instead of boxy. A whole fly is 2.3 mm: keep the poly counts
  // small or 70 parts stop being free.
  function ellipsoid(frame, k, material, seg = 16) {
    const g = track(new THREE.SphereGeometry(1, seg, Math.max(8, seg >> 1)));
    const m = new THREE.Mesh(g, material);
    m.scale.set(frame.s[0] * k[0] * 0.5, frame.s[1] * k[1] * 0.5, frame.s[2] * k[2] * 0.5);
    m.position.set(frame.c[0], frame.c[1], frame.c[2]);
    return m;
  }

  // A capsule along whichever axis the segment is longest on, tapering with
  // radius from the two short axes. Covers legs, haltere stalks, aristas.
  // lenScale shortens only the long axis (and its offset from the node), so a
  // limb can be made stubby without losing its girth.
  function longCapsule(frame, radiusK, material, lenScale = 1) {
    const { s, c } = frame;
    const axis = s[0] >= s[1] && s[0] >= s[2] ? 0 : (s[1] >= s[2] ? 1 : 2);
    const shortAxes = [0, 1, 2].filter((i) => i !== axis);
    const len = s[axis] * lenScale;
    const rad = (s[shortAxes[0]] + s[shortAxes[1]]) * 0.25 * radiusK;
    const body = Math.max(len - 2 * rad, len * 0.05);
    const g = track(new THREE.CapsuleGeometry(rad, body, 4, detailed ? 10 : 6));
    const m = new THREE.Mesh(g, material);
    // CapsuleGeometry is built along +Y; rotate it onto the long axis.
    if (axis === 0) m.rotation.z = Math.PI / 2;
    else if (axis === 2) m.rotation.x = Math.PI / 2;
    const cc = c.slice();
    cc[axis] *= lenScale;                   // keep the near end at the joint
    m.position.set(cc[0], cc[1], cc[2]);
    return m;
  }

  // ------------------------------------------------------------------ build
  // Every left-side part gains a mirrored right-side twin (legs are declared
  // symmetric in the rig, but eyes, wings, halteres and antennae are not).
  const allShorts = new Set(Object.keys(STL_FRAMES));
  for (const s of Object.keys(STL_FRAMES)) {
    if (/^(l_|l[fhm]_)/.test(s)) allShorts.add('r' + s.slice(1));
  }

  for (const short of [...allShorts].sort()) {
    const frame = frameFor(short);
    if (!frame) continue;
    const kind = kindOf(short);
    const group = new THREE.Group();
    group.name = 'cute:' + short;

    switch (kind) {
      case 'thorax': {
        group.add(ellipsoid(frame, [1.0, 0.96, 0.92], chitin, 20));
        // A warm scutellum patch over the rear third and a bristle tuft: reads
        // as a fuzzy fly rather than a smooth pebble.
        group.add(ellipsoid({ s: frame.s, c: [frame.c[0] - 0.16, 0, frame.c[2] + 0.02] },
          [0.44, 0.66, 0.62], chitinWarm, 14));
        const nb = detailed ? 7 : 3;
        for (let i = 0; i < nb; i++) {
          const a = (i / nb) * Math.PI * 2;
          const g = track(new THREE.ConeGeometry(0.008, 0.07, 4));
          const b = new THREE.Mesh(g, chitinPale);
          b.position.set(frame.c[0] - 0.10 + 0.16 * Math.cos(a), 0.17 * Math.sin(a), frame.c[2] + 0.30);
          b.rotation.z = -0.5 - 0.25 * Math.cos(a);
          b.rotation.x = 0.3 * Math.sin(a);
          group.add(b);
        }
        break;
      }
      case 'head':
        group.add(ellipsoid(frame, [0.96, 0.94, 0.92], chitin, 16));
        break;
      case 'eye':
        // Oversized on purpose: k > 1 pushes the eye well past the real
        // cuticle until it dominates the head, which is the single biggest
        // cuteness signal for an insect face.
        group.add(ellipsoid(frame, [1.28, 1.62, 1.34], eyeMat, 18));
        break;
      case 'abdomen': {
        // Alternate warm and dark bands along the abdomen.
        const warm = /(3|5)$/.test(short);
        group.add(ellipsoid(frame, [0.95, 0.94, 0.92], warm ? chitinWarm : chitin, 16));
        break;
      }
      case 'proboscis':
        group.add(ellipsoid(frame, [0.9, 0.9, 0.9], chitinPale, 10));
        break;
      case 'antenna':
        group.add(ellipsoid(frame, [0.95, 0.95, 0.95], chitinPale, 10));
        break;
      case 'arista':
        group.add(longCapsule(frame, 0.5, chitinPale));
        break;
      case 'haltere': {
        // Thin stalk out to a bright amber knob.
        const axis = frame.s[1] >= frame.s[2] ? 1 : 2;
        const len = frame.s[axis];
        const sg = track(new THREE.CapsuleGeometry(0.016, len * 0.55, 3, 6));
        const stalk = new THREE.Mesh(sg, haltereMat);
        if (axis === 2) stalk.rotation.x = Math.PI / 2;
        stalk.position.set(frame.c[0], frame.c[1] * 0.35, frame.c[2]);
        group.add(stalk);
        const kg = track(new THREE.SphereGeometry(0.075, 12, 8));
        const knob = new THREE.Mesh(kg, haltereMat);
        knob.position.set(frame.c[0], frame.c[1] * 0.95, frame.c[2]);
        group.add(knob);
        break;
      }
      case 'wing': {
        // Flat blade: same footprint as the real wing, almost no thickness.
        // SphereGeometry puts its poles on +-Y, and the wing's long axis IS Y,
        // so the texture's vertical lines run base-to-tip as veins should.
        const g = track(new THREE.SphereGeometry(1, detailed ? 24 : 12, detailed ? 28 : 14));
        const w = new THREE.Mesh(g, wingMat);
        w.scale.set(frame.s[0] * 0.52, frame.s[1] * 0.5, 0.055);
        w.position.set(frame.c[0], frame.c[1], frame.c[2]);
        w.renderOrder = 2;
        group.add(w);
        break;
      }
      case 'coxaseg':
      case 'legseg':
        // Stubby on purpose. Anatomical legs are fine wires as long as the
        // body; a cartoon fly has short thick limbs, so the radius is inflated
        // well past the mesh and the length cut back to LEG_SEG_SCALE.
        group.add(longCapsule(frame, kind === 'coxaseg' ? 1.45 : 1.25, legMat, LEG_SEG_SCALE));
        break;
      default:
        group.add(ellipsoid(frame, [0.92, 0.92, 0.92], chitinPale, 10));
    }
    parts[short] = group;
  }

  return {
    parts,
    stats: { nodes: Object.keys(parts).length, geometries: geos.length, materials: mats.length },
    dispose() {
      for (const g of geos) g.dispose();
      for (const m of mats) { if (m.map && m.map.dispose) m.map.dispose(); m.dispose(); }
    },
  };
}

// Wing membrane with longitudinal veins and a darker leading edge. Rendered on
// the sphere UV, where v runs base (1) to tip (0).
function wingVeinTexture(THREE) {
  const W = 256, H = 256;
  const cv = (typeof document !== 'undefined') ? document.createElement('canvas') : null;
  if (!cv) return null;
  cv.width = W; cv.height = H;
  const g = cv.getContext('2d');
  g.clearRect(0, 0, W, H);
  g.fillStyle = '#e9f2ff';
  g.fillRect(0, 0, W, H);
  // Longitudinal veins: curves running the full height, bulging outwards.
  g.strokeStyle = '#5a4230';
  g.lineWidth = 3;
  const veins = [0.16, 0.30, 0.44, 0.58, 0.72, 0.86];
  for (let i = 0; i < veins.length; i++) {
    const x0 = veins[i] * W;
    const bulge = (i % 2 === 0 ? 1 : -1) * (6 + i * 2);
    g.beginPath();
    g.moveTo(x0, H);
    g.quadraticCurveTo(x0 + bulge, H * 0.5, x0 + bulge * 0.4, 0);
    g.stroke();
  }
  // Cross veins near the middle.
  g.lineWidth = 2;
  g.beginPath();
  g.moveTo(0.16 * W, 0.62 * H);
  g.quadraticCurveTo(0.5 * W, 0.55 * H, 0.86 * W, 0.66 * H);
  g.stroke();
  g.beginPath();
  g.moveTo(0.30 * W, 0.34 * H);
  g.quadraticCurveTo(0.5 * W, 0.30 * H, 0.72 * W, 0.36 * H);
  g.stroke();
  // Darker leading edge along one side.
  g.strokeStyle = '#3d2c1e';
  g.lineWidth = 7;
  g.beginPath();
  g.moveTo(0.04 * W, H);
  g.lineTo(0.04 * W, 0);
  g.stroke();
  const tex = new THREE.CanvasTexture(cv);
  tex.colorSpace = THREE.SRGBColorSpace || undefined;
  tex.wrapS = THREE.RepeatWrapping;
  tex.anisotropy = 4;
  return tex;
}
