#!/bin/sh
# scripts/life-vithi-check.sh: read-only check of Steve's newest life vithi records.
# Reads only state_dir/life/journal.jsonl. Does not write, and does not open memory.
# Stage order is life's. Curiosity puts determining before investigation.
# A feeling summary must be exactly "feeling-tone: uncalibrated, not shown".
set -eu

SELF_TEST=0
for arg in "$@"; do
    case "$arg" in
        --self-test)
            SELF_TEST=1
            ;;
        -h|--help)
            echo "Usage: $0 [--self-test]"
            echo "Reads the newest curiosity, mail_walk and dream entries from the live container."
            echo "Default container: ferricula-steve. Override with FERRICULA_CONTAINER."
            echo "Default journal: /data/agent-runtime/life/journal.jsonl (FERRICULA_LIFE_JOURNAL_PATH)."
            echo "FERRICULA_LIFE_JOURNAL reads a host file instead, still read-only."
            exit 0
            ;;
        *)
            echo "Unknown argument: $arg" >&2
            echo "Usage: $0 [--self-test]" >&2
            exit 1
            ;;
    esac
done

if [ "$SELF_TEST" -eq 0 ]; then
    CONTAINER="${FERRICULA_CONTAINER:-ferricula-steve}"
    JOURNAL_PATH="${FERRICULA_LIFE_JOURNAL_PATH:-/data/agent-runtime/life/journal.jsonl}"
    case "$CONTAINER" in
        *[!A-Za-z0-9_.-]*)
            echo "FAIL: container name has unexpected characters" >&2
            exit 1
            ;;
    esac
    case "$JOURNAL_PATH" in
        *agent-memory*|*steve-memory*|*wal.log*|*snapshot*)
            echo "FAIL: refusing to read a memory path" >&2
            exit 1
            ;;
        /data/agent-runtime/life/journal.jsonl) ;;
        *)
            echo "FAIL: journal path must be /data/agent-runtime/life/journal.jsonl" >&2
            exit 1
            ;;
    esac
    if [ -n "${FERRICULA_LIFE_JOURNAL:-}" ]; then
        case "$FERRICULA_LIFE_JOURNAL" in
            *agent-memory*|*steve-memory*|*wal.log*|*snapshot*)
                echo "FAIL: refusing to read a memory path" >&2
                exit 1
                ;;
        esac
        if [ ! -f "$FERRICULA_LIFE_JOURNAL" ]; then
            echo "FAIL: journal file not found" >&2
            exit 1
        fi
        JOURNAL_FILE=$FERRICULA_LIFE_JOURNAL
        CLEAN_JOURNAL=0
    else
        if ! command -v docker >/dev/null 2>&1; then
            echo "FAIL: docker is not available" >&2
            exit 1
        fi
        JOURNAL_FILE=$(mktemp)
        CLEAN_JOURNAL=1
        trap 'if [ "${CLEAN_JOURNAL:-0}" -eq 1 ]; then rm -f "$JOURNAL_FILE"; fi' EXIT INT TERM
        if ! docker exec "$CONTAINER" cat -- "$JOURNAL_PATH" > "$JOURNAL_FILE"; then
            echo "FAIL: could not read the life journal from $CONTAINER" >&2
            exit 1
        fi
    fi
    export LIFE_VITHI_JOURNAL=$JOURNAL_FILE
    export LIFE_VITHI_SELF_TEST=0
else
    export LIFE_VITHI_JOURNAL=
    export LIFE_VITHI_SELF_TEST=1
fi

python3 - <<'PY'
import json
import os
import sys

KINDS = ("curiosity", "mail_walk", "dream")
FIELDS = ("stage", "pali", "measured", "summary", "detail", "advisory")
# Life's order, not the chat turn's. curiosity puts determining before investigation.
EXPECTED = {
    "curiosity": (
        "contact", "feeling", "recognition", "determining",
        "investigation", "impulsion", "registration",
    ),
    "mail_walk": ("contact", "determining", "impulsion", "registration"),
    "dream": ("contact", "recognition", "impulsion", "registration"),
}
FEELING_SUMMARY = "feeling-tone: uncalibrated, not shown"


def has_key(obj, key):
    if isinstance(obj, dict):
        if key in obj:
            return True
        return any(has_key(value, key) for value in obj.values())
    if isinstance(obj, list):
        return any(has_key(value, key) for value in obj)
    return False


def check_entry(kind, entry):
    if not isinstance(entry, dict):
        return "FAIL %s: entry is not an object" % kind
    if "vithi" not in entry or entry.get("vithi") is None:
        return "FAIL %s: vithi missing" % kind
    vithi = entry.get("vithi")
    if not isinstance(vithi, list):
        return "FAIL %s: vithi is not an array" % kind
    if not vithi:
        return "FAIL %s: vithi is empty" % kind
    saw_feeling = False
    for index, stage in enumerate(vithi):
        if not isinstance(stage, dict):
            return "FAIL %s: vithi[%d] is not an object" % (kind, index)
        for field in FIELDS:
            if field not in stage:
                return "FAIL %s: vithi[%d] missing %s" % (kind, index, field)
        name = stage.get("stage")
        pali = stage.get("pali")
        if not isinstance(name, str) or name == "":
            return "FAIL %s: vithi[%d] stage is empty" % (kind, index)
        if not isinstance(pali, str) or pali == "":
            return "FAIL %s: vithi[%d] pali is empty" % (kind, index)
        if not isinstance(stage.get("measured"), bool):
            return "FAIL %s: vithi[%d] measured is not a boolean" % (kind, index)
        summary = stage.get("summary")
        if not isinstance(summary, str) or summary.strip() == "":
            return "FAIL %s: vithi[%d] summary is empty" % (kind, index)
        if not isinstance(stage.get("detail"), dict):
            return "FAIL %s: vithi[%d] detail is not an object" % (kind, index)
        if not isinstance(stage.get("advisory"), bool):
            return "FAIL %s: vithi[%d] advisory is not a boolean" % (kind, index)
        if name == "feeling":
            saw_feeling = True
            if stage.get("measured") is not False:
                return "FAIL %s: feeling measured is not false" % kind
            if has_key(stage, "valence"):
                return "FAIL %s: feeling has valence" % kind
            if summary != FEELING_SUMMARY:
                return "FAIL %s: feeling summary is not the uncalibrated line" % kind
    if kind == "curiosity" and not saw_feeling:
        return "FAIL %s: feeling stage missing" % kind
    names = tuple(item.get("stage") for item in vithi)
    expected = EXPECTED[kind]
    if names != expected:
        return "FAIL %s: stages %s, expected %s" % (kind, list(names), list(expected))
    return "PASS %s" % kind


def newest(text):
    found = {}
    for raw in text.splitlines():
        line = raw.strip()
        if not line:
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(obj, dict) and obj.get("kind") in KINDS:
            found[obj["kind"]] = obj
    return found


def report(text):
    found = newest(text)
    lines = []
    failed = False
    for kind in KINDS:
        if kind not in found:
            lines.append("FAIL %s: no %s entry in the journal" % (kind, kind))
            failed = True
            continue
        row = check_entry(kind, found[kind])
        lines.append(row)
        if not row.startswith("PASS "):
            failed = True
    return lines, failed


def stage(name, measured=True, summary="recorded", detail=None, advisory=False):
    return {
        "stage": name,
        "pali": "pali",
        "measured": measured,
        "summary": summary,
        "detail": {} if detail is None else detail,
        "advisory": advisory,
    }


def dump(entries):
    return "\n".join(json.dumps(entry) for entry in entries)


def self_test():
    feeling = stage(
        "feeling",
        measured=False,
        summary="feeling-tone: uncalibrated, not shown",
        detail={"reason": "no calibrated feeling-tone gate"},
        advisory=True,
    )
    def vithi_for(kind, names=None):
        chosen = EXPECTED[kind] if names is None else names
        return [feeling if name == "feeling" else stage(name) for name in chosen]

    good = [
        {"kind": "curiosity", "vithi": vithi_for("curiosity")},
        {"kind": "mail_walk", "vithi": vithi_for("mail_walk")},
        {"kind": "dream", "vithi": vithi_for("dream")},
    ]
    # An older broken curiosity must not hide the newest good one.
    older = {"kind": "curiosity", "vithi": [stage("contact")]}
    lines, failed = report(dump([older] + good))
    expect = ["PASS curiosity", "PASS mail_walk", "PASS dream"]
    if lines != expect or failed:
        print("FAIL self-test: good journal -> %s" % lines)
        return 1

    mail = good[1]
    dream = good[2]
    curiosity = good[0]
    broken_dream = {
        "kind": "dream",
        "vithi": [{**stage("registration"), "detail": None}],
    }
    cases = [
        ([{**curiosity, "vithi": None}, mail, dream], "curiosity"),
        ([{**curiosity, "vithi": [stage("feeling", measured=False, summary="   ")]}, mail, dream], "curiosity"),
        ([{"kind": "curiosity", "vithi": [stage("feeling", measured=True, summary="shown")]}, mail, dream], "curiosity"),
        ([{"kind": "curiosity", "vithi": [stage("feeling", measured=False, summary="shown", detail={"valence": 0})]}, mail, dream], "curiosity"),
        ([{"kind": "curiosity", "vithi": [stage("contact")]}, mail, dream], "curiosity"),
        ([curiosity, {"kind": "mail_walk"}, dream], "mail_walk"),
        ([curiosity, mail, broken_dream], "dream"),
        ([curiosity, {"kind": "mail_walk", "vithi": [stage("feeling", measured=True, summary="shown")]}, dream], "mail_walk"),
        ([
            {
                "kind": "curiosity",
                "vithi": vithi_for("curiosity", (
                    "contact", "feeling", "recognition", "investigation",
                    "determining", "impulsion", "registration",
                )),
            },
            mail,
            dream,
        ], "curiosity"),
        ([
            curiosity,
            mail,
            {"kind": "dream", "vithi": vithi_for("dream", ("contact", "impulsion", "registration"))},
        ], "dream"),
    ]
    for entries, kind in cases:
        bad_lines, bad_failed = report(dump(entries))
        hit = next(line for line in bad_lines if line.startswith("FAIL %s:" % kind) or line.startswith("PASS %s" % kind))
        if not bad_failed or not hit.startswith("FAIL %s:" % kind):
            print("FAIL self-test: expected %s to fail, got %s" % (kind, bad_lines))
            return 1
    for line in lines:
        print(line)
    print("self-test ok")
    return 0


def main():
    if os.environ.get("LIFE_VITHI_SELF_TEST") == "1":
        return self_test()
    path = os.environ.get("LIFE_VITHI_JOURNAL", "")
    if not path:
        print("FAIL: journal path missing")
        return 1
    with open(path, "r", encoding="utf-8") as handle:
        text = handle.read()
    lines, failed = report(text)
    for line in lines:
        print(line)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
PY
