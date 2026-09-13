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
the exact root IDs and cell types, so the simulation can wire a real sensory
channel instead of inventing one.

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

Taxonomy notes (measured, not assumed)
--------------------------------------
`class`      mechanosensory | mechanosensory_proprioceptive |
             mechanosensory_tactile | mechanosensory_tbc
`subclass`   chordotonal organ 425 | campaniform sensilla 426 | hair plate 113 |
             mechanosensory bristle 2206 | wind_gravity 475 | auditory 114 |
             notum 226 | leg 403 | haltere 201 | grooming 65
`entryNerve` ProLN/MesoLN/MetaLN = pro/meso/metathoracic leg nerves,
             ADMN + PDMN = dorsal mesothoracic (wing), DMetaN = dorsal
             metathoracic (haltere), AN = antennal (Johnston's organ).

Two traps this script encodes:
  * subclass 'wing bristle' (385 neurons) is class == gustatory, i.e. wing TASTE
    bristles, not mechanosensory. Never group it with the sense organs above.
  * a neuron can have a NULL class. Comparisons must treat NULL as "no match"
    (SQL semantics), which is what dropping nulls before the bitwise AND does.
"""

import argparse
import hashlib
import json
import os
import sys

LEG_NERVES = '{DProN, MesoLN, MetaLN, PrN, ProAN, ProCN, ProLN, VProN}'

# (group_name, [left selector, right selector], biological_scope, evidence)
# Selector clauses are combined with 'and'.
GROUPS = [
    ("leg_chordotonal", [
        "status == Traced and subclass == chordotonal organ and rootSide == L",
        "status == Traced and subclass == chordotonal organ and rootSide == R"],
     "leg chordotonal organs (joint angle and vibration)", "measured_annotation"),

    ("leg_hair_plate", [
        "status == Traced and subclass == hair plate and rootSide == L",
        "status == Traced and subclass == hair plate and rootSide == R"],
     "leg hair plates (joint extremes, posture)", "measured_annotation"),

    ("leg_proprioceptive", [
        f"status == Traced and class == mechanosensory_proprioceptive and entryNerve in {LEG_NERVES} and rootSide == L",
        f"status == Traced and class == mechanosensory_proprioceptive and entryNerve in {LEG_NERVES} and rootSide == R"],
     "leg proprioception (chordotonal + hair plate + campaniform + leg)",
     "measured_annotation"),

    ("leg_tactile", [
        f"status == Traced and class == mechanosensory_tactile and entryNerve in {LEG_NERVES} and rootSide == L",
        f"status == Traced and class == mechanosensory_tactile and entryNerve in {LEG_NERVES} and rootSide == R"],
     "leg tactile bristles (contact with surfaces)", "measured_annotation"),

    ("wing_mechanosensory", [
        "status == Traced and class == mechanosensory_proprioceptive and entryNerve == ADMN and rootSide == L",
        "status == Traced and class == mechanosensory_proprioceptive and entryNerve == ADMN and rootSide == R"],
     "wing campaniform sensilla and stretch receptors (wing load)",
     "measured_annotation"),

    ("notum_tactile", [
        "status == Traced and class == mechanosensory_tactile and subclass == notum and rootSide == L",
        "status == Traced and class == mechanosensory_tactile and subclass == notum and rootSide == R"],
     "notum / wing-base tactile bristles", "measured_annotation"),

    ("haltere_proprioceptive", [
        "status == Traced and subclass == haltere and rootSide == L",
        "status == Traced and subclass == haltere and rootSide == R"],
     "haltere campaniform sensilla (body rotation rate)", "measured_annotation"),

    ("haltere_ascending", [
        "status == Traced and superclass == sensory_ascending and subclass == haltere and rootSide == L",
        "status == Traced and superclass == sensory_ascending and subclass == haltere and rootSide == R"],
     "haltere afferents ascending to the brain", "measured_annotation"),

    ("antennal_wind_gravity", [
        "status == Traced and class == mechanosensory and subclass == wind_gravity and entryNerve == AN and rootSide == L",
        "status == Traced and class == mechanosensory and subclass == wind_gravity and entryNerve == AN and rootSide == R"],
     "Johnston's organ, wind and gravity sensing", "measured_annotation"),

    ("antennal_auditory", [
        "status == Traced and class == mechanosensory and subclass == auditory and entryNerve == AN and rootSide == L",
        "status == Traced and class == mechanosensory and subclass == auditory and entryNerve == AN and rootSide == R"],
     "Johnston's organ, auditory", "measured_annotation"),

    ("abdominal_proprioceptive", [
        "status == Traced and class == mechanosensory_proprioceptive and subclass == abdomen and rootSide == L",
        "status == Traced and class == mechanosensory_proprioceptive and subclass == abdomen and rootSide == R"],
     "abdominal proprioception (segmental stretch)", "measured_annotation"),

    ("body_tactile", [
        "status == Traced and class == mechanosensory_tactile and rootSide == L",
        "status == Traced and class == mechanosensory_tactile and rootSide == R"],
     "whole-body tactile bristles (includes leg and notum groups)",
     "measured_annotation"),

    ("ascending_proprioceptive", [
        "status == Traced and superclass == sensory_ascending and rootSide == L",
        "status == Traced and superclass == sensory_ascending and rootSide == R"],
     "all ascending VNC to brain sensory feedback", "measured_annotation"),
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
        for prefix, field, op in (
            ('class == ', 'class', '=='),
            ('class starts ', 'class', 'starts'),
            ('subclass == ', 'subclass', '=='),
            ('subclass in ', 'subclass', 'in'),
            ('superclass == ', 'superclass', '=='),
            ('entryNerve in ', 'entryNerve', 'in'),
            ('entryNerve == ', 'entryNerve', '=='),
            ('status == ', 'status', '=='),
            ('rootSide == ', 'rootSide', '=='),
        ):
            if tok.startswith(prefix):
                out.append((field, op, tok[len(prefix):]))
                break
        else:
            raise SystemExit('unparsed selector clause: ' + tok)
    return out


def to_set(value):
    v = value.strip()
    if v.startswith('{') and v.endswith('}'):
        return [x.strip() for x in v[1:-1].split(',') if x.strip()]
    return [v]


def row_matches(row, clauses):
    # A NULL field never matches, mirroring SQL: drop nulls before the AND.
    for field, op, value in clauses:
        cur = row.get(field)
        if cur is None:
            return False
        cur = str(cur)
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

    cols = ['bodyId', 'status', 'class', 'subclass', 'superclass', 'entryNerve',
            'rootSide', 'type']
    table = feather.read_table(args.annotations, columns=cols)
    rows = table.to_pylist()
    print(f'annotation rows: {len(rows)}')
    if pack_ids is not None:
        print(f'pack neurons:    {len(pack_ids)}')

    groups = {}
    for name, selectors, scope, evidence in GROUPS:
        for side, selector in zip(('left', 'right'), selectors):
            key = f'{name}_{"L" if side == "left" else "R"}'
            clauses = parse_clause_set(selector)
            hits = [r for r in rows if row_matches(r, clauses)]
            ids = set(int(r['bodyId']) for r in hits)
            in_pack = sorted(ids & pack_ids) if pack_ids is not None else sorted(ids)
            cell_types = sorted(set(
                str(r['type']) for r in hits
                if r['type'] is not None and str(r['type']).strip()))
            groups[key] = {
                'biological_scope': scope,
                'evidence_category': evidence,
                'cell_types': cell_types,
                'pack_resolution': (
                    f'{len(in_pack)} of {len(ids)} selected neurons are present in the '
                    f'compiled connectome' if pack_ids is not None
                    else 'pack not supplied; root_ids are the full annotated set'),
                'root_ids': in_pack,
                'selector': selector,
                'side': side,
            }
            print(f'  {key:34s} selected={len(ids):5d}  in_pack={len(in_pack):5d}  '
                  f'types={len(cell_types):3d}')

    all_ids = set()
    overlaps = {}
    for k, g in groups.items():
        for i in g['root_ids']:
            if i in all_ids:
                overlaps[i] = overlaps.get(i, 0) + 1
            all_ids.add(i)

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
                     'report body state. Same group schema and selector style. Every count, '
                     'root_id and cell type is measured from the annotation table. Groups '
                     'overlap by design: body_tactile contains leg_tactile and notum_tactile, '
                     'and ascending_proprioceptive contains the ascending subsets.'),
        },
        'groups': groups,
        'summary': {
            'group_count': len(groups),
            'groups_with_zero_pack_neurons': [k for k, g in groups.items() if not g['root_ids']],
            'total_root_ids': sum(len(g['root_ids']) for g in groups.values()),
            'unique_root_ids': len(all_ids),
            'root_ids_shared_between_groups': len(overlaps),
        },
    }

    os.makedirs(os.path.dirname(args.out) or '.', exist_ok=True)
    with open(args.out, 'w') as fh:
        json.dump(doc, fh, indent=1)
        fh.write('\n')

    s = doc['summary']
    print(f"\nwrote {args.out}: {s['group_count']} groups, "
          f"{s['total_root_ids']} root ids ({s['unique_root_ids']} unique, "
          f"{s['root_ids_shared_between_groups']} shared)")
    if s['groups_with_zero_pack_neurons']:
        print('WARNING groups with no pack neurons: '
              + ', '.join(s['groups_with_zero_pack_neurons']))
    return 0


if __name__ == '__main__':
    sys.exit(main())
