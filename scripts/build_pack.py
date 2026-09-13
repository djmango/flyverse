"""Compile the official Janelia MaleCNS v1.0 release into a CSR connectome.

Inputs are the three files published by FlyEM at
gs://flyem-male-cns/v1.0/connectome-data/flat-connectome/ :
  body-annotations-...feather        neuron annotations
  body-neurotransmitters-...feather  consensus neurotransmitter per neuron
  connectome-weights-...feather      the full segment-to-segment graph (1.05 GB, 151.8M rows)

Output: a pack directory with neuron_ids / row_ptr / destinations / signed_counts
plus a manifest recording the source SHA-256 of every input.

The code here is ours. The data is Janelia's.
"""
from __future__ import annotations

import hashlib
import json
import os
import sys
import time
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.feather as feather

ROOT = Path("/opt/data/workspaces/skg/flybrain")
OUT = Path(os.environ.get("FLYVERSE_PACK_OUT", ROOT / "official-pack"))

# Engineering convention for the sign of an edge, taken from the transmitter of
# the presynaptic cell. Only transmitters with a settled sign are used; anything
# else drops the outgoing edge of that neuron.
#
# histamine is in the negative set, and leaving it out was a real bug: the fly's
# photoreceptors are histaminergic, so excluding it silently deleted the whole
# retinal output. 491,144 photoreceptor edges existed in the raw release and
# none of them reached the pack, which left the eye connected to nothing and
# made the optic lobe unreachable from the world. Histamine is a fast inhibitory
# transmitter in the lamina: it hyperpolarises the lamina monopolar cells
# through HisCl1, a chloride channel, so it belongs with gaba and glutamate
# rather than with acetylcholine.
#
# dopamine, octopamine and serotonin stay out. They are neuromodulators whose
# sign is not a fixed excitatory/inhibitory property of a single synapse, so
# there is no defensible value to give them.
POSITIVE = {"acetylcholine"}
NEGATIVE = {"gaba", "glutamate", "histamine"}


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> None:
    t0 = time.perf_counter()

    ann_path = ROOT / "annotations.feather"
    nt_path = ROOT / "official-neurotransmitters.feather"
    conn_path = ROOT / "official-connectivity.feather"

    sources = {p.name: sha256(p) for p in (ann_path, nt_path, conn_path)}
    print("source hashes:", json.dumps(sources, indent=1), flush=True)

    ann = feather.read_table(ann_path, columns=["bodyId", "superclass", "type", "subclass", "somaSide"])
    keep_mask = pc.is_valid(ann.column("superclass"))
    ann = ann.filter(keep_mask)
    body_ids = ann.column("bodyId").to_numpy().astype(np.uint64)
    order = np.argsort(body_ids, kind="stable")
    body_ids = body_ids[order]
    ann = ann.take(pa.array(order))
    print(f"annotated neurons with a superclass: {body_ids.size:,}", flush=True)

    nt = feather.read_table(nt_path, columns=["body", "consensus_nt"])
    nt_ids = nt.column("body").to_numpy().astype(np.uint64)
    nt_labels = np.asarray(nt.column("consensus_nt").to_pylist(), dtype=object)

    # sign per annotated neuron, looked up by body id
    label_of: dict[int, str] = {int(b): (l if l else "") for b, l in zip(nt_ids, nt_labels)}
    signs = np.zeros(body_ids.size, dtype=np.int8)
    for i, b in enumerate(body_ids):
        lab = label_of.get(int(b), "")
        if lab in POSITIVE:
            signs[i] = 1
        elif lab in NEGATIVE:
            signs[i] = -1
    known = int(np.count_nonzero(signs))
    print(f"neurons with a signed consensus transmitter: {known:,}", flush=True)

    # stream the connection graph, keep edges whose two ends are annotated
    import pyarrow.ipc as ipc

    reader = ipc.open_file(pa.memory_map(str(conn_path), "r"))
    pre_parts: list[np.ndarray] = []
    post_parts: list[np.ndarray] = []
    w_parts: list[np.ndarray] = []
    rows = 0
    kept = 0
    for i in range(reader.num_record_batches):
        batch = reader.get_batch(i)
        if i and i % 400 == 0:
            print(f"  batch {i}/{reader.num_record_batches} ({time.perf_counter() - t0:.0f}s)", flush=True)
        pre = batch.column("body_pre").to_numpy().astype(np.uint64)
        post = batch.column("body_post").to_numpy().astype(np.uint64)
        w = batch.column("weight").to_numpy().astype(np.int64)
        rows += batch.num_rows
        pi = np.searchsorted(body_ids, pre)
        qi = np.searchsorted(body_ids, post)
        ok = (pi < body_ids.size) & (qi < body_ids.size)
        ok &= body_ids[np.minimum(pi, body_ids.size - 1)] == pre
        ok &= body_ids[np.minimum(qi, body_ids.size - 1)] == post
        ok &= w > 0
        pre_parts.append(pi[ok].astype(np.uint32))
        post_parts.append(qi[ok].astype(np.uint32))
        w_parts.append(w[ok].astype(np.int32))
        kept += int(ok.sum())
    del reader

    pre_idx = np.concatenate(pre_parts)
    post_idx = np.concatenate(post_parts)
    weight = np.concatenate(w_parts)
    del pre_parts, post_parts, w_parts
    print(f"raw rows {rows:,}; edges with both ends annotated: {kept:,} "
          f"({time.perf_counter() - t0:.0f}s)", flush=True)

    # drop outgoing edges of neurons without a settled transmitter sign
    src_sign = signs[pre_idx]
    signed = weight.astype(np.int64) * src_sign
    edge_ok = src_sign != 0
    pre_idx = pre_idx[edge_ok]
    post_idx = post_idx[edge_ok]
    signed = signed[edge_ok]
    print(f"edges with a known presynaptic transmitter: {pre_idx.size:,}", flush=True)

    print(f"signed_counts range: {int(signed.min()):,} to {int(signed.max()):,}", flush=True)
    print(f"destinations range: {int(post_idx.min()):,} to {int(post_idx.max()):,}", flush=True)
    if signed.min() < -32768 or signed.max() > 32767:
        raise SystemExit("signed_counts does not fit in int16")
    if post_idx.max() >= body_ids.size:
        raise SystemExit("destination index outside the neuron table")
    signed = signed.astype(np.int16)
    post_idx = post_idx.astype(np.uint32)

    # CSR: sort by source
    order = np.argsort(pre_idx, kind="stable")
    pre_sorted = pre_idx[order]
    destinations = post_idx[order]
    signed_counts = signed[order]
    del order, pre_idx, post_idx, signed

    row_ptr = np.zeros(body_ids.size + 1, dtype=np.uint64)
    np.cumsum(np.bincount(pre_sorted, minlength=body_ids.size), out=row_ptr[1:])
    row_ptr = row_ptr.astype(np.uint32)

    OUT.mkdir(parents=True, exist_ok=True)
    np.save(OUT / "neuron_ids.npy", body_ids)
    np.save(OUT / "row_ptr.npy", row_ptr)
    np.save(OUT / "destinations.npy", destinations)
    np.save(OUT / "signed_counts.npy", signed_counts)

    manifest = {
        "dataset": "MaleCNS",
        "dataset_version": "v1.0",
        "source": "gs://flyem-male-cns/v1.0/connectome-data/flat-connectome/",
        "license": "CC BY 4.0",
        "compiled_by": "our own compiler (official_build.py); no third-party pack used",
        "transmitter_sign_policy": {"positive": sorted(POSITIVE), "negative": sorted(NEGATIVE)},
        "counts": {
            "neurons": int(body_ids.size),
            "neurons_with_signed_transmitter": known,
            "raw_connectivity_rows": int(rows),
            "edges": int(destinations.size),
            "excitatory_edges": int(np.count_nonzero(signed_counts > 0)),
            "inhibitory_edges": int(np.count_nonzero(signed_counts < 0)),
            "contact_sum": int(np.abs(signed_counts.astype(np.int64)).sum()),
        },
        "source_sha256": sources,
        "array_sha256": {
            n: hashlib.sha256((OUT / n).read_bytes()).hexdigest()
            for n in ("neuron_ids.npy", "row_ptr.npy", "destinations.npy", "signed_counts.npy")
        },
    }
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2))
    print(json.dumps(manifest["counts"], indent=2), flush=True)
    print(f"done in {time.perf_counter() - t0:.0f}s -> {OUT}", flush=True)


if __name__ == "__main__":
    sys.exit(main())
