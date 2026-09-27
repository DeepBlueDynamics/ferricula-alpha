#!/usr/bin/env python3
"""Recompute run summaries for runs recorded before labels were carried inline.

Joins run JSONL rows with the dataset by id and rewrites the trailing summary
line in place. Usage:
  python3 resummarize.py --gate vedana --dataset ../datasets/vedana_en.v1.jsonl ../runs/*.jsonl
"""
import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gate_runner import summarize, norm_label, GATES  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--gate", required=True, choices=list(GATES))
    ap.add_argument("--dataset", required=True)
    ap.add_argument("runs", nargs="+")
    args = ap.parse_args()

    label_key = GATES[args.gate]["label_key"]
    labels = {}
    for ln in open(args.dataset, encoding="utf-8"):
        if ln.strip():
            r = json.loads(ln)
            labels[r["id"]] = norm_label(args.gate, r[label_key])

    for path in args.runs:
        lines = [l for l in open(path, encoding="utf-8") if l.strip()]
        rows = []
        header = None
        for ln in lines:
            r = json.loads(ln)
            if "summary" in r:
                continue
            if "id" not in r:
                header = r
                continue
            r["label"] = labels.get(r["id"])
            rows.append(r)
        summary = summarize(rows, args.gate)
        with open(path, "w", encoding="utf-8") as f:
            if header:
                f.write(json.dumps(header, ensure_ascii=False) + "\n")
            for r in rows:
                f.write(json.dumps(r, ensure_ascii=False) + "\n")
            f.write(json.dumps({"summary": summary, "resummarized": True},
                               ensure_ascii=False) + "\n")
        print(f"== {path}")
        print(json.dumps(summary, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
