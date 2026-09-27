#!/usr/bin/env python3
"""Gate runner — measures Abhidhamma decision gates against labelled JSONL sets.

Implements the Gates-lane contract (research/2026-09-26-gates.md):
- provider-neutral chat-model backends via Ollama's /v1/chat/completions
- typed answer OR typed abstention, never a fabricated judgment
- full provenance per row: model, ts, latency, token usage, raw response
- probabilities are *self-reported*, never claimed calibrated (calibration is a
  separate held-out step in calibrate.py per Eldest Dog's discipline)

Usage:
  python3 gate_runner.py --gate vedana --lang en --dataset ../datasets/vedana_en.v1.jsonl \
      --model kimi-k3:cloud --out ../runs/vedana_en_kimi-k3_$(date +%H%M%S).jsonl
"""
import argparse
import json
import os
import re
import sys
import time
import urllib.request
import urllib.error
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
PROMPTS = HERE.parent / "prompts"

GATES = {
    "vedana": {
        "classes": ["sukha", "dukkha", "adukkhamasukha"],
        "extra_keys": {"intensity", "tibbatā"},
        "label_key": "valence",
        # 3 classes: abstain if p_max below 1/3 + 0.17 margin
        "confidence_floor": 0.50,
    },
    "sanna": {
        "classes": ["KO", "KIM", "KADA", "KATTHA", "KASMA", "KATHAM"],
        "extra_keys": set(),
        "label_key": "sanna",
        # 6 classes: abstain if p_max below 1/6 + ~0.17 margin
        "confidence_floor": 0.34,
    },
}

# Pāḷi prompt keys → canonical class names
PA_KEYMAP = {"ko": "KO", "kiṁ": "KIM", "kadā": "KADA", "kattha": "KATTHA",
             "kasmā": "KASMA", "kathaṁ": "KATHAM"}


def norm_label(gate: str, label):
    if label is None:
        return None
    if gate == "sanna":
        return PA_KEYMAP.get(str(label).strip(), str(label).strip())
    return label


def load_jsonl(path):
    rows = []
    with open(path, encoding="utf-8") as f:
        for ln in f:
            ln = ln.strip()
            if ln:
                rows.append(json.loads(ln))
    return rows


def extract_json(raw: str):
    s = raw.strip()
    m = re.search(r"```(?:json)?\s*(\{.*\})\s*```", s, re.S)
    if m:
        s = m.group(1)
    start = s.find("{")
    if start < 0:
        return None
    depth = 0
    for i in range(start, len(s)):
        if s[i] == "{":
            depth += 1
        elif s[i] == "}":
            depth -= 1
            if depth == 0:
                try:
                    return json.loads(s[start:i + 1])
                except json.JSONDecodeError:
                    return None
    return None


def call_model(host, model, prompt, timeout=90):
    body = {"model": model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.0}
    req = urllib.request.Request(
        host + "/v1/chat/completions", data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"})
    t0 = time.perf_counter()
    with urllib.request.urlopen(req, timeout=timeout) as r:
        resp = json.load(r)
    dt_ms = (time.perf_counter() - t0) * 1000.0
    return resp, dt_ms


def validate_probs(obj, classes):
    probs = obj.get("probs")
    if not isinstance(probs, dict):
        return None, "missing probs object"
    normed = {}
    for c in classes:
        v = probs.get(c)
        if v is None:
            return None, f"missing class {c}"
        try:
            v = float(v)
        except (TypeError, ValueError):
            return None, f"class {c} not numeric"
        if not (0.0 <= v <= 1.0):
            return None, f"class {c}={v} out of [0,1]"
        normed[c] = v
    total = sum(normed.values())
    if not (0.95 <= total <= 1.05):
        return None, f"probs sum {total:.4f} outside [0.95,1.05]"
    return {c: v / total for c, v in normed.items()}, None


def judge_row(host, model, gate, template, row):
    rec = {"ts": datetime.now(timezone.utc).isoformat(),
           "gate": gate, "model": model, "id": row["id"],
           "label": norm_label(gate, row.get(GATES[gate]["label_key"]))}
    g = GATES[gate]
    prompt = template.replace("{text}", row["text"])
    try:
        resp, dt = call_model(host, model, prompt)
    except (urllib.error.URLError, TimeoutError, OSError) as e:
        rec.update({"verdict": None, "abstain": {"reason": "provider_error",
                                                 "message": str(e)[:200]}})
        return rec
    rec["latency_ms"] = round(dt, 1)
    usage = resp.get("usage") or {}
    rec["usage"] = {"input": usage.get("prompt_tokens"),
                    "output": usage.get("completion_tokens")}
    raw = (resp["choices"][0]["message"].get("content") or "")
    rec["raw"] = raw[:2000]

    obj = extract_json(raw)
    if obj is None:
        rec.update({"verdict": None,
                    "abstain": {"reason": "invalid_output", "message": "no JSON object"}})
        return rec
    if gate == "sanna" and any(k in obj.get("probs", {}) for k in PA_KEYMAP):
        obj["probs"] = {PA_KEYMAP.get(k, k): v for k, v in obj["probs"].items()}
    probs, err = validate_probs(obj, g["classes"])
    if err:
        rec.update({"verdict": None,
                    "abstain": {"reason": "invalid_output", "message": err}})
        return rec
    p_max = max(probs.values())
    pred = max(probs, key=probs.get)
    verdict = {"probs": probs, "pred": pred, "p_max": round(p_max, 6)}
    if gate == "vedana":
        iv = obj.get("intensity", obj.get("tibbatā"))
        try:
            iv = int(iv)
            if not (0 <= iv <= 4):
                raise ValueError
        except (TypeError, ValueError):
            rec.update({"verdict": None, "abstain": {"reason": "invalid_output",
                                                     "message": f"intensity {iv!r} not int in 0-4"}})
            return rec
        verdict["intensity"] = iv
    if p_max < g["confidence_floor"]:
        rec.update({"verdict": None, "abstain": {"reason": "low_confidence",
                                                 "p_max": round(p_max, 6)},
                    "probs": probs})
        return rec
    rec["verdict"] = verdict
    rec["abstain"] = None
    return rec


def summarize(run_rows, gate):
    g = GATES[gate]
    n = len(run_rows)
    labelled = [r for r in run_rows]
    answered = [r for r in labelled if r.get("verdict")]
    abstains = [r for r in labelled if r.get("abstain")]
    lat = sorted(r["latency_ms"] for r in labelled if "latency_ms" in r)
    p50 = lat[len(lat) // 2] if lat else None
    p95 = lat[min(len(lat) - 1, int(0.95 * len(lat)))] if lat else None
    labels = [r.get("label") for r in answered]
    if any(l is None for l in labels):
        raise SystemExit("run rows missing labels; rerun with fixed judge_row")
    preds = [r["verdict"]["pred"] for r in answered]
    classes = g["classes"]
    acc = sum(1 for a, b in zip(labels, preds) if a == b) / max(1, len(answered))
    f1s = []
    for c in classes:
        tp = sum(1 for a, b in zip(labels, preds) if a == c and b == c)
        fp = sum(1 for a, b in zip(labels, preds) if a != c and b == c)
        fn = sum(1 for a, b in zip(labels, preds) if a == c and b != c)
        if tp + fp + fn:
            p = tp / (tp + fp) if tp + fp else 0.0
            r_ = tp / (tp + fn) if tp + fn else 0.0
            f1s.append(2 * p * r_ / (p + r_) if p + r_ else 0.0)
        else:
            f1s.append(None)
    macro = (sum(f for f in f1s if f is not None) / max(1, sum(1 for f in f1s if f is not None)))
    # ECE on answered rows, 10 bins of confidence
    bins = [[0.0, 0.0, 0] for _ in range(10)]
    for r, a in zip(answered, labels):
        c = r["verdict"]["p_max"]
        ok = 1.0 if r["verdict"]["pred"] == a else 0.0
        i = min(9, int(c * 10))
        bins[i][0] += c
        bins[i][1] += ok
        bins[i][2] += 1
    ece = sum((cnt / max(1, len(answered))) * abs(b1 / cnt - b0 / cnt)
              for b0, b1, cnt in bins if cnt)
    return {"n": n, "answered": len(answered), "abstained": len(abstains),
            "accuracy_on_answered": round(acc, 4), "macro_f1": round(macro, 4),
            "ece_selfreported": round(ece, 4),
            "latency_p50_ms": p50, "latency_p95_ms": p95,
            "per_class_f1": {c: (round(f, 4) if f is not None else None)
                             for c, f in zip(classes, f1s)},
            "abstain_reasons": {r: sum(1 for x in abstains if x["abstain"]["reason"] == r)
                                for r in {x["abstain"]["reason"] for x in abstains}}}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--gate", required=True, choices=list(GATES))
    ap.add_argument("--lang", required=True)
    ap.add_argument("--dataset", required=True)
    ap.add_argument("--model", default="kimi-k3:cloud")
    ap.add_argument("--out", required=True)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--sleep", type=float, default=0.0)
    ap.add_argument("--resume", action="store_true",
                    help="append to --out; skip row ids already judged in that file")
    args = ap.parse_args()

    host = os.environ.get("OLLAMA_HOST", "http://127.0.0.1:11434")
    template = (PROMPTS / f"gate_{args.gate}_{args.lang}.txt").read_text(encoding="utf-8")
    rows = load_jsonl(args.dataset)
    if args.limit:
        rows = rows[: args.limit]

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    results = []
    done_ids = set()
    if args.resume and out_path.exists():
        with open(out_path, encoding="utf-8") as old:
            for ln in old:
                if ln.strip():
                    try:
                        r = json.loads(ln)
                    except json.JSONDecodeError:
                        continue
                    if "id" in r and "summary" not in r:
                        results.append(r)
                        done_ids.add(r["id"])
        # Drop any trailing summary line from a previous partial run.
        rows = [r for r in rows if r["id"] not in done_ids]
    with open(out_path, "a" if args.resume else "w", encoding="utf-8") as out:
        if not args.resume or not done_ids:
            import hashlib
            ds_sha = hashlib.sha256(Path(args.dataset).read_bytes()).hexdigest()
            out.write(json.dumps({
                "command": " ".join(sys.argv), "model": args.model, "gate": args.gate,
                "lang": args.lang, "dataset": args.dataset, "dataset_sha256": ds_sha,
                "labels_source": "externally authored, source-group-separated",
                "started": datetime.now(timezone.utc).isoformat()}, ensure_ascii=False) + "\n")
        for i, row in enumerate(rows):
            rec = judge_row(host, args.model, args.gate, template, row)
            rec["i"] = i
            out.write(json.dumps(rec, ensure_ascii=False) + "\n")
            out.flush()
            results.append(rec)
            v = rec["verdict"]["pred"] if rec.get("verdict") else f"ABSTAIN:{rec['abstain']['reason']}"
            print(f"[{i+1:3d}/{len(rows)}] {row['id']} -> {v} ({rec.get('latency_ms','-')}ms)")
            if args.sleep:
                time.sleep(args.sleep)
        summary = summarize(results, args.gate)
        out.write(json.dumps({"summary": summary}, ensure_ascii=False) + "\n")
    print(json.dumps(summary, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
