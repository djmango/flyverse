#!/usr/bin/env python3
"""Mine the real mechanosensory / proprioceptive neuron populations out of the
MaleCNS v1.0 annotation table and write them as flyverse stimulus groups.

Why this exists
---------------
flyverse can already drive identified sensory neurons for olfaction, taste and
vision (assets/male_cns_v1_neural_io.json). Closing the loop on a moving body
also needs the sense organs that report what the body is DOING: leg
chordotonal organs, hair plates, campaniform sensilla, haltere afferents and
Johnston's organ. This script finds them in the annotation table and records
the exact root IDs, so the simulation can wire a real sensory channel instead
of inventing one.

Requires the neuPrint annotation export (annotations.feather), which is NOT in
this repo -- download it from the MaleCNS v1.0 release. The connectome pack is
optional: when present, each group is also intersected with the body IDs the
compiled graph actually contains, because a neuron that is not in the graph
cannot be stimulated.

    python3 scripts/mine_mechanosensory.py \
        --annotations /path/to/annotations.feather \
        --pack /path/to/official-pack/neuron_ids.npy \
        --out assets/male_cns_v1_mechanosensory_io.json

Every count written is measured from the annotation table; nothing is assumed.
Output schema matches assets/male_cns_v1_neural_io.json.
"""

import argparse
import hashlib
import json
import os
import sys

# (group_name, side, biological_scope, evidence_category, [selector clauses])
# entryNerve is the strongest anatomical discriminator in this release:
#   ProLN / MesoLN / MetaLN = pro-, meso-, metathoracic leg nerves
#   ADMN / PDMN             = dorsal mesothoracic nerves (wing)
#   DMetaN                  = dorsal metathoracic nerve (haltere)
#   AN                      = antennal nerve (Johnston's organ)
#   AbN3                    = third abdominal nerve
GROUPS = [
    ("leg_proprioceptive_pro", "left", "prothoracic leg proprioception",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and entryNerve in {ProLN} "
     "and subclass in {chordotonal organ, hair plate} and rootSide == L"),
    ("leg_proprioceptive_pro", "right", "prothoracic leg proprioception",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and entryNerve in {ProLN} "
     "and subclass in {chordotonal organ, hair plate} and rootSide == R"),
    ("leg_proprioceptive_meso", "left", "mesothoracic leg proprioception",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and entryNerve in {MesoLN} "
     "and subclass in {chordotonal organ, hair plate} and rootSide == L"),
    ("leg_proprioceptive_meso", "right", "mesothoracic leg proprioception",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and entryNerve in {MesoLN} "
     "and subclass in {chordotonal organ, hair plate} and rootSide == R"),
    ("leg_proprioceptive_meta", "left", "metathoracic leg proprioception",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and entryNerve in {MetaLN} "
     "and subclass in {chordotonal organ, hair plate} and rootSide == L"),
    ("leg_proprioceptive_meta", "right", "metathoracic leg proprioception",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and entryNerve in {MetaLN} "
     "and subclass in {chordotonal organ, hair plate} and rootSide == R"),
    ("leg_campaniform_pro", "left", "prothoracic leg campaniform sensilla",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and entryNerve in {ProLN} "
     "and subclass == campaniform sensilla and rootSide == L"),
    ("leg_campaniform_meso", "left", "mesothoracic leg campaniform sensilla",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and entryNerve in {MesoLN} "
     "and subclass == campaniform sensilla and rootSide == L"),
    ("leg_campaniform_meta", "left", "metathoracic leg campaniform sensilla",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and entryNerve in {MetaLN} "
     "and subclass == campaniform sensilla and rootSide == L"),
    ("leg_tactile", "left", "leg tactile bristles",
     "measured_annotation",
     "status == Traced and class == mechanosensory_tactile and entryNerve in "
     "{ProLN, MesoLN, MetaLN} and rootSide == L"),
    ("leg_tactile", "right", "leg tactile bristles",
     "measured_annotation",
     "status == Traced and class == mechanosensory_tactile and entryNerve in "
     "{ProLN, MesoLN, MetaLN} and rootSide == R"),
    ("wing_mechanosensory", "left", "wing campaniform and stretch receptors",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and entryNerve in {ADMN, PDMN} "
     "and rootSide == L"),
    ("wing_mechanosensory", "right", "wing campaniform and stretch receptors",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and entryNerve in {ADMN, PDMN} "
     "and rootSide == R"),
    ("haltere_mechanosensory", "left", "haltere campaniform sensilla",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and entryNerve in {DMetaN} "
     "and rootSide == L"),
    ("haltere_mechanosensory", "right", "haltere campaniform sensilla",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and entryNerve in {DMetaN} "
     "and rootSide == R"),
    ("antennal_johnston", "left", "Johnston's organ (auditory + wind/gravity)",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and entryNerve in {AN} "
     "and rootSide == L"),
    ("antennal_johnston", "right", "Johnston's organ (auditory + wind/gravity)",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and entryNerve in {AN} "
     "and rootSide == R"),
    ("abdominal_mechanosensory", "left", "abdominal mechanosensory",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and entryNerve in {AbN3} "
     "and rootSide == L"),
    ("abdominal_mechanosensory", "right", "abdominal mechanosensory",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and entryNerve in {AbN3} "
     "and rootSide == R"),
    ("notum_mechanosensory", "left", "notum campaniform and bristles",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and subclass == notum "
     "and rootSide == L"),
    ("notum_mechanosensory", "right", "notum campaniform and bristles",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and subclass == notum "
     "and rootSide == R"),
    ("body_tactile", "left", "body tactile bristles (grooming / general)",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and subclass in {grooming, abdomen} "
     "and rootSide == L"),
    ("body_tactile", "right", "body tactile bristles (grooming / general)",
     "measured_annotation",
     "status == Traced and class starts mechanosensory and subclass in {grooming, abdomen} "
     "and rootSide == R"),
]


def sha256(path):
    h = hashlib.sha256()
    with open(path, 'rb') as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b''):
            h.update(chunk)
    return h.hexdigest()


def parse_clause_set(text):
    """Pull 'field op value' clauses out of a selector string."""
    out = []
    for tok in text.split(' and '):
        tok = tok.strip()
        if not tok:
            continue
        if tok.startswith('class == '):
            out.append(('class', '==', tok[len('class == '):]))
        elif tok.startswith('class starts '):
            out.append(('class', 'starts', tok[len('class starts '):]))
        elif tok.startswith('subclass == '):
            out.append(('subclass', '==', tok[len('subclass == '):]))
        elif tok.startswith('subclass in '):
            out.append(('subclass', 'in', tok[len('subclass in '):]))
        elif tok.startswith('entryNerve in '):
            out.append(('entryNerve', 'in', tok[len('entryNerve in '):]))
        elif tok.startswith('entryNerve == '):
            out.append(('entryNerve', '==', tok[len('entryNerve == '):]))
        elif tok.startswith('status == '):
            out.append(('status', '==', tok[len('status == '):]))
        elif tok.startswith('rootSide == '):
            out.append(('rootSide', '==', tok[len('rootSide == '):]))
        else:
            raise SystemExit('unparsed selector clause: ' + tok)
    return out


def to_set(value):
    v = value.strip()
    if v.startswith('{') and v.endswith('}'):
        return [x.strip() for x in v[1:-1].split(',') if x.strip()]
    return [v]


def row_matches(row, clauses):
    for field, op, value in clauses:
        cur = row.get(field)
        cur = '' if cur is None else str(cur)
        if op == '==':
            if cur != value:
                return False
        elif op == 'starts':
            if not cur.startswith(value):
                return False
        elif op == 'in':
            if cur not in to_set(value):
                return False
    return True


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--annotations', required=True, help='annotations.feather')
    ap.add_argument('--pack', help='official-pack/neuron_ids.npy (optional)')
    ap.add_argument('--out', default='assets/male_cns_v1_mechanosensory_io.json')
    args = ap.parse_args()

    try:
        import numpy as np
        import pyarrow.feather as feather
    except ImportError as e:
        raise SystemExit('needs numpy and pyarrow: ' + str(e))

    pack_ids = None
    if args.pack and os.path.exists(args.pack):
        pack_ids = set(int(x) for x in np.load(args.pack))

    cols = ['bodyId', 'status', 'class', 'subclass', 'entryNerve', 'rootSide']
    table = feather.read_table(args.annotations, columns=cols)
    rows = table.to_pylist()
    print(f'annotation rows: {len(rows)}')
    if pack_ids is not None:
        print(f'pack neurons:    {len(pack_ids)}')

    groups = {}
    for name, side, scope, evidence, selector in GROUPS:
        key = f'{name}_{"L" if side == "left" else "R"}'
        clauses = parse_clause_set(selector)
        hits = [int(r['bodyId']) for r in rows if row_matches(r, clauses)]
        in_pack = sorted(set(hits) & pack_ids) if pack_ids is not None else sorted(set(hits))
        groups[key] = {
            'biological_scope': scope,
            'evidence_category': evidence,
            'pack_resolution': (f'{len(in_pack)} of {len(set(hits))} annotated neurons are present '
                                f'in the compiled connectome' if pack_ids is not None
                                else 'pack not supplied; root_ids are the full annotated set'),
            'root_ids': in_pack,
            'selector': selector,
            'side': side,
        }
        print(f'  {key:34s} annotated={len(set(hits)):5d}  in_pack={len(in_pack):5d}')

    doc = {
        'schema_version': 1,
        'dataset': {
            'name': 'MaleCNS v1.0 mechanosensory / proprioceptive sensory inventory',
            'annotation_source': os.path.basename(args.annotations),
            'annotation_sha256': sha256(args.annotations),
            'annotation_rows': len(rows),
            'pack_source': os.path.basename(args.pack) if args.pack else None,
            'pack_neuron_count': len(pack_ids) if pack_ids is not None else None,
            'note': ('Extends assets/male_cns_v1_neural_io.json with the sense organs that '
                     'report body state. Same group schema and selector style. Every count '
                     'and root_id is measured from the annotation table.'),
        },
        'groups': groups,
        'summary': {
            'group_count': len(groups),
            'groups_with_zero_pack_neurons': [k for k, g in groups.items() if not g['root_ids']],
            'total_root_ids': sum(len(g['root_ids']) for g in groups.values()),
        },
    }

    os.makedirs(os.path.dirname(args.out) or '.', exist_ok=True)
    with open(args.out, 'w') as fh:
        json.dump(doc, fh, indent=1)
        fh.write('\n')
    empty = doc['summary']['groups_with_zero_pack_neurons']
    print(f"\nwrote {args.out}: {len(groups)} groups, "
          f"{doc['summary']['total_root_ids']} root ids")
    if empty:
        print('WARNING groups with no pack neurons: ' + ', '.join(empty))
    return 0


if __name__ == '__main__':
    sys.exit(main())
