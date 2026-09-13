# Third-party notices and attribution

The MIT license in [LICENSE](LICENSE) covers the original code and documentation of
this repository. It does not relicense the bundled third-party data or assets below,
which keep their own terms. This is a mixed-license distribution, not an assertion
that every included file is MIT or commercially reusable.

## Component inventory

| Component | Files | Upstream terms | Required credit |
|---|---|---|---|
| MaleCNS v1.0 connectome data and annotations | `data/targets_vnc_sensory.u64`, `data/soma_positions.f32`, `data/superclass_ids.json`, `assets/male_cns_v1_neural_io.json` | **CC BY 4.0** | FlyEM / HHMI Janelia, Cambridge, MRC LMB, Google Research, and the MaleCNS collaboration/publication. Release terms: https://male-cns.janelia.org/download/ |
| NeuroMechFly / FlyGym body, gait and mesh assets | `web/assets/meshes/*.stl` (39 files), `web/assets/rig.json` | **Apache-2.0** | The FlyGym / NeuroMechFly authors. Cite both NeuroMechFly papers. FlyGym retains an upstream Apache-2.0 license and includes inherited components attributed by that project. |
| three.js 0.180.0 (module, core, STLLoader) | `web/vendor/three.module.js`, `web/vendor/three.core.js`, `web/vendor/STLLoader.js` | **MIT** | Copyright 2010-2025 three.js authors. License header retained in each file. https://github.com/mrdoob/three.js |
| Room backdrop art | `web/assets/oak-v1.png` | Original work | Generated for this project; covered by the repository MIT license. |
| Connectome pack (not committed) | compiled separately | **CC BY 4.0** | Derived from the MaleCNS v1.0 release above. `official-pack/manifest.json` records the license, counts and the SHA-256 of every source and derived array. |

## Data pipeline note

The connectome pack is compiled from the public flat-connectome release at
`gs://flyem-male-cns/v1.0/connectome-data/flat-connectome/`. No third-party
pre-built pack is redistributed. Transmitter sign policy applied during compilation:
acetylcholine positive; GABA and glutamate negative.

## Modifications

The connectome is converted to signed CSR arrays with neuron-ID mappings. Public
annotations are selected into stimulus and motor read-out groups by
`prep/prep_inputs.py`. Body meshes are used unmodified; `web/assets/rig.json` is a
derived rest-pose table for the web renderer.
