#!/usr/bin/env python3
"""Nightly gate calibration (plan §6.2) and held-out ECE (Eldest Dog discipline).

Logit recovery follows the plan verbatim: Ollaya returns calibrated
probabilities p = softmax(z / T), so z ∝ T·log p up to a per-row constant.
For chat backends the role of p is played by the model's *self-reported*
option distribution; Eldest Dog's directive applies: self-reported confidence
is not a recovered posterior, so calibration is only ever claimed after a
held-out measurement of ECE < 0.05.

Splitting is by source group (label provenance `source` field), never by row,
so the same memory/author family never appears on both sides (plan §5.3
spirit: split by source_memory_id).

Acceptance (plan §6.3): new temperatures accepted only if dev ECE improves
and accuracy does not drop. Output format matches the plan's CALIBRATION json.

Usage:
  python3 calibrate.py --gate vedana --runs ../runs/vedana_en_kimi-k3_*.jsonl \
      --dataset ../datasets/vedana_en.v1.jsonl --out ../calibration/vedana.json
"""
import argparse
import glob
import json
from collections import defaultdict
from pathlib import Path

import numpy as np
from scipy.optimize import minimize_scalar

CLASSES = {
    "vedana": ["sukha", "dukkha", "adukkhamasukha"],
    "sanna": ["KO", "KIM", "KADA", "KATTHA", "KASMA", "KATHAM"],
}
LABEL_KEY = {"vedana": "valence", "sanna": "sanna"}
PA_KEYMAP = {"ko": "KO", "kiṁ": "KIM", "kadā": "KADA", "kattha": "KATTHA",
             "kasmā": "KASMA", "kathaṁ": "KATHAM"}


def norm_label(gate, label):
    if gate == "sanna" and label is not None:
        return PA_KEYMAP.get(str(label).strip(), str(label).strip())
    return label


def load_run(path):
    rows = []
    with open(path, encoding="utf-8") as f:
        for ln in f:
            if ln.strip():
                r = json.loads(ln)
                if "id" in r:
                    rows.append(r)
    return rows


def fit_temperature(probs, labels, T_current=1.0):
    """plan §6.2 verbatim: z ∝ T·log p; fit T' minimizing NLL."""
    z = T_current * np.log(np.clip(probs, 1e-9, 1.0))

    def nll(T):
        s = z / T
        s = s - s.max(axis=1, keepdims=True)
        logp = s - np.log(np.exp(s).sum(axis=1, keepdims=True))
        return -logp[np.arange(len(labels)), labels].mean()

    return minimize_scalar(nll, bounds=(0.05, 20.0), method="bounded").x


def ece(probs, labels, n_bins=10):
    conf = probs.max(axis=1)
    pred = probs.argmax(axis=1)
    ok = (pred == labels).astype(float)
    total = len(labels)
    e = 0.0
    for lo in np.linspace(0, 1, n_bins, endpoint=False):
        hi = lo + 1.0 / n_bins
        m = (conf >= lo) & (conf < hi if hi < 1.0 else conf <= hi)
        if m.any():
            e += m.mean() * abs(ok[m].mean() - conf[m].mean())
    return float(e)


def accuracy(probs, labels):
    return float((probs.argmax(axis=1) == labels).mean())


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--gate", required=True, choices=list(CLASSES))
    ap.add_argument("--runs", required=True, help="glob of run JSONL files")
    ap.add_argument("--dataset", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--ece_target", type=float, default=0.05)
    args = ap.parse_args()

    classes = CLASSES[args.gate]
    cidx = {c: i for i, c in enumerate(classes)}
    label_key = LABEL_KEY[args.gate]

    labels = {}
    with open(args.dataset, encoding="utf-8") as f:
        for ln in f:
            if ln.strip():
                r = json.loads(ln)
                labels[r["id"]] = (norm_label(args.gate, r[label_key]), r.get("source", "unknown"))

    # Merge runs; last verdict for a row id wins (rerun overrides).
    seen = {}
    for path in sorted(glob.glob(args.runs)):
        for r in load_run(path):
            if r.get("verdict"):
                seen[r["id"]] = r["verdict"]["probs"]
    ids = sorted(set(seen) & set(labels))
    if len(ids) < 12:
        raise SystemExit(f"need >=12 judged+labelled rows, got {len(ids)}")

    # Split by source *family*: everything before ':' in the label source.
    groups = defaultdict(list)
    for i in ids:
        # Family = first whitespace token of the label's source string:
        # "authored: boundary" → "authored", "composed parallel sen0001" → "composed".
        fam = labels[i][1].split()[0] if labels[i][1].split() else "unknown"
        groups[fam].append(i)
    # Largest families first: the biggest family is train, the rest dev/test.
    fams = sorted(groups, key=lambda f: -len(groups[f]))
    if len(fams) < 3:
        # Fall back to interleaved thirds by stable id hash — flagged in output.
        import hashlib
        def stable(x):
            return int.from_bytes(hashlib.sha256(x.encode()).digest()[:4], "big") % 997
        ids_sorted = sorted(ids, key=lambda x: (stable(x), x))
        splits = {"train": ids_sorted[0::3], "dev": ids_sorted[1::3], "test": ids_sorted[2::3]}
        split_note = "row-hash thirds (fewer than 3 source families)"
    else:
        splits = {"train": groups[fams[0]], "dev": groups[fams[1]],
                  "test": [i for f in fams[2:] for i in groups[f]]}
        split_note = f"source families (size desc): train={fams[0]} dev={fams[1]} test={fams[2:]}"

    def mat(id_list):
        P = np.array([[seen[i][c] for c in classes] for i in id_list])
        L = np.array([cidx[labels[i][0]] for i in id_list])
        return P, L

    P_tr, L_tr = mat(splits["train"])
    P_dev, L_dev = mat(splits["dev"])
    P_te, L_te = mat(splits["test"])

    T = fit_temperature(np.vstack([P_tr, P_dev]), np.concatenate([L_tr, L_dev]))

    def softmax_T(P, T):
        z = np.log(np.clip(P, 1e-9, 1.0)) / T
        z = z - z.max(axis=1, keepdims=True)
        e = np.exp(z)
        return e / e.sum(axis=1, keepdims=True)

    report = {
        "gate": args.gate,
        "score_provenance": ("model-reported (verbalized probability at "
                             "temperature 0) — NOT logit-derived; calibration "
                             "discipline per Eldest Dog 2026-09-26"),
        "n_judged_labelled": len(ids),
        "split": {k: len(v) for k, v in splits.items()},
        "split_note": split_note,
        "split_disjoint_by": "label source family (source field before ':')",
        "temperature_fitted": round(float(T), 4),
        "held_out": {
            "ece_raw": round(ece(P_te, L_te), 4),
            "ece_calibrated": round(ece(softmax_T(P_te, T), L_te), 4),
            "accuracy_raw": round(accuracy(P_te, L_te), 4),
            "accuracy_calibrated": round(accuracy(softmax_T(P_te, T), L_te), 4),
        },
        "dev": {
            "ece_raw": round(ece(P_dev, L_dev), 4),
            "ece_calibrated": round(ece(softmax_T(P_dev, T), L_dev), 4),
        },
    }
    he = report["held_out"]
    he["n"] = len(splits["test"])
    # Plan §5 calibration must generalize: a held-out set smaller than 20 rows
    # cannot demonstrate ECE < target, it can only happen to pass.
    dev_improves = report["dev"]["ece_calibrated"] <= report["dev"]["ece_raw"] + 1e-9
    accepted = (he["ece_calibrated"] < args.ece_target
                and he["accuracy_calibrated"] >= he["accuracy_raw"] - 1e-9
                and he["n"] >= 20
                and dev_improves)
    report["dev_ece_improved"] = bool(dev_improves)
    report["lifecycle_authorized"] = bool(accepted)
    report["note"] = (
        "calibrated probabilities never claimed from this run" if not accepted
        else "held-out ECE < target; calibration file may drive lifecycle")
    out = {"temperature": [round(float(T), 4)] * len(classes),
           "acceptance": report}
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False, indent=2)
    import hashlib
    digest = hashlib.sha256(Path(args.out).read_bytes()).hexdigest()
    report["artifact_path"] = str(Path(args.out).resolve())
    report["artifact_sha256"] = digest
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
