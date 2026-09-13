#!/usr/bin/env node
// Prints the local bounding frame of every anatomical fly mesh, in millimetres.
//
// web/fly_cute.js embeds this table as STL_FRAMES and uses it as a size
// template: the cartoon body is built from primitives sized to occupy the same
// volume as the real mesh, so it lands correctly on the shared rig. Re-run this
// after changing web/assets/meshes/ and paste the output over STL_FRAMES.
//
//   node scripts/mesh_frames.cjs
//
// s = bounding-box size [x, y, z], c = bounding-box centre [x, y, z]. The STLs
// are stored in metres; the renderer scales them by 1000, so this reports mm.

const fs = require('fs');
const path = require('path');

const dir = path.join(__dirname, '..', 'web', 'assets', 'meshes');
const files = fs.readdirSync(dir).filter((f) => f.endsWith('.stl')).sort();
if (!files.length) {
  console.error('no .stl files in ' + dir);
  process.exit(1);
}

const rows = [];
for (const f of files) {
  const buf = fs.readFileSync(path.join(dir, f));
  const tris = buf.readUInt32LE(80);
  const min = [1e9, 1e9, 1e9];
  const max = [-1e9, -1e9, -1e9];
  for (let i = 0; i < tris; i++) {
    const base = 84 + i * 50 + 12;          // skip normal + 12 bytes of float
    for (let v = 0; v < 3; v++) {
      for (let a = 0; a < 3; a++) {
        const val = buf.readFloatLE(base + v * 12 + a * 4);
        if (val < min[a]) min[a] = val;
        if (val > max[a]) max[a] = val;
      }
    }
  }
  const mm = (v) => Math.round(v * 1e6) / 1e3;   // metres -> mm, 3 dp
  const s = [0, 1, 2].map((a) => mm(max[a] - min[a]));
  const c = [0, 1, 2].map((a) => mm((max[a] + min[a]) / 2));
  rows.push('  ' + f.replace('.stl', '') +
    ': { s: [' + s.join(', ') + '], c: [' + c.join(', ') + '] },');
}

console.log('export const STL_FRAMES = {');
console.log(rows.join('\n'));
console.log('};');
