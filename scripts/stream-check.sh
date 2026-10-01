#!/bin/sh
# scripts/stream-check.sh: live check of the Ferricula chat SSE turn stream.
set -e

NO_STAGES=0
for arg in "$@"; do
    case "$arg" in
        --no-stages)
            NO_STAGES=1
            ;;
        -h|--help)
            echo "Usage: $0 [--no-stages]"
            exit 0
            ;;
        *)
            echo "Unknown argument: $arg" >&2
            echo "Usage: $0 [--no-stages]" >&2
            exit 1
            ;;
    esac
done

FERRICULA_URL="${FERRICULA_URL:-http://127.0.0.1:18875}"
FERRICULA_URL="${FERRICULA_URL%/}"

# Read operator token from file named by $FERRICULA_OPERATOR_TOKEN_FILE
if [ -n "$FERRICULA_OPERATOR_TOKEN_FILE" ]; then
    if [ ! -f "$FERRICULA_OPERATOR_TOKEN_FILE" ]; then
        echo "FAIL: Token file '$FERRICULA_OPERATOR_TOKEN_FILE' not found"
        exit 1
    fi
    TOKEN=$(cat "$FERRICULA_OPERATOR_TOKEN_FILE" | tr -d '\r\n')
elif [ -n "$FERRICULA_OPERATOR_TOKEN" ]; then
    TOKEN="$FERRICULA_OPERATOR_TOKEN"
else
    echo "FAIL: FERRICULA_OPERATOR_TOKEN_FILE environment variable not set"
    exit 1
fi

if [ -z "$TOKEN" ]; then
    echo "FAIL: Operator token is empty"
    exit 1
fi

PAYLOAD=$(python3 -c "import json, uuid; print(json.dumps({
    'request_id': str(uuid.uuid4()),
    'conversation_id': str(uuid.uuid4()),
    'message': 'ping',
    'reported_origin': 'human'
}))")

# Run curl and stream SSE output directly into python validator
curl -sSN -w "\n__HTTP_STATUS__:%{http_code}\n" \
    --connect-timeout 10 \
    -X POST \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "$PAYLOAD" \
    "$FERRICULA_URL/chat/stream" | python3 -c '
import sys
import json

no_stages = ("--no-stages" in sys.argv[1:])

events = []
current_event = None
current_data = []
http_status = None
non_sse_lines = []
header_printed = False

def print_row(event_name, t_val, summary):
    global header_printed
    if not header_printed:
        col_event = "EVENT"
        col_t = "T (ms)"
        print(f"{col_event:<18} {col_t:>8}   SUMMARY")
        print("-" * 78)
        header_printed = True
    print(f"{event_name:<18} {str(t_val):>8}   {summary}", flush=True)

def make_summary(event_name, data):
    if not isinstance(data, dict):
        return str(data)[:70]
    if event_name == "stage":
        stage = data.get("stage", "")
        summary = data.get("summary", "")
        measured = data.get("measured")
        if measured is False:
            prefix = f"{stage} (unmeasured)" if stage else "unmeasured"
            return f"{prefix}: {summary}" if summary else prefix
        prefix = stage if stage else ""
        return f"{prefix}: {summary}" if (prefix and summary) else (summary or prefix)
    if event_name == "accepted":
        cid = data.get("conversation_id", "")
        return f"conversation_id: {cid}"
    if event_name == "candidate":
        cand = data.get("candidate", {})
        if isinstance(cand, dict):
            text = cand.get("text", "")
            cid = cand.get("id", "")
            return f"recalled: {text[:50]}" if text else f"id: {cid}"
        return str(cand)[:50]
    if event_name == "document":
        card = data.get("card", {})
        if isinstance(card, dict):
            title = card.get("title") or card.get("url") or card.get("name") or ""
            return f"card: {title[:50]}" if title else str(card)[:50]
        return str(card)[:50]
    if event_name == "curator":
        ok = data.get("ok", "")
        skipped = data.get("skipped", False)
        return f"ok={ok} skipped={skipped}"
    if event_name == "round_start":
        r = data.get("round", "")
        toks = data.get("prompt_tokens_est", "")
        return f"round {r} prompt_tokens_est={toks}"
    if event_name == "round":
        r = data.get("round", "")
        finish = data.get("finish", "")
        secs = data.get("secs", "")
        return f"round {r} finish={finish} ({secs}s)"
    if event_name == "tool_call":
        name = data.get("name", "")
        n = data.get("n", "")
        return f"tool {name} (#{n})"
    if event_name == "tool_result":
        n = data.get("n", "")
        secs = data.get("secs", "")
        return f"tool result #{n} ({secs}s)"
    if event_name == "gate":
        gate = data.get("gate", "")
        kind = data.get("kind", "")
        return f"gate {gate} ({kind})"
    if event_name == "verdict":
        res = data.get("result", "")
        return f"verdict: {res}"
    if event_name == "nudge":
        reason = data.get("reason", "")
        return f"nudge: {reason}"
    if event_name == "done":
        secs = data.get("secs", "")
        turn = data.get("turn", {})
        status = turn.get("status", "") if isinstance(turn, dict) else ""
        return f"completed in {secs}s status={status}"
    if event_name == "failed":
        err = data.get("error", "")
        secs = data.get("secs", "")
        return f"failed ({secs}s): {err}"
    if "summary" in data:
        return str(data["summary"])[:70]
    if "error" in data:
        err = data["error"]
        return f"error: {err}"[:70]
    keys = [f"{k}={v}" for k, v in data.items() if k not in ("event", "request_id", "t") and not isinstance(v, (dict, list))]
    return ", ".join(keys[:4])[:70]

def dispatch_event(event_name, data_str):
    try:
        payload = json.loads(data_str)
    except Exception:
        payload = {"raw": data_str}
    ev = event_name or (payload.get("event") if isinstance(payload, dict) else "message")
    t_val = payload.get("t", "-") if isinstance(payload, dict) else "-"
    summary = make_summary(ev, payload)
    print_row(ev, t_val, summary)
    events.append({"event": ev, "data": payload})
    return ev in ("done", "failed")

for line in sys.stdin:
    line = line.rstrip("\r\n")
    if line.startswith("__HTTP_STATUS__:"):
        try:
            http_status = int(line.split(":", 1)[1].strip())
        except ValueError:
            pass
        continue
    if not line:
        if current_data:
            should_stop = dispatch_event(current_event, "\n".join(current_data))
            current_event = None
            current_data = []
            if should_stop:
                break
        continue
    if line.startswith(":"):
        continue
    if line.startswith("event:"):
        current_event = line[6:].strip()
    elif line.startswith("data:"):
        current_data.append(line[5:].lstrip())
    else:
        non_sse_lines.append(line)

if current_data:
    dispatch_event(current_event, "\n".join(current_data))

if not events:
    if http_status == 401:
        err_msg = "\n".join(non_sse_lines) or "operator authorization required"
        print(f"FAIL: server returned HTTP 401 Unauthorized: {err_msg}")
        sys.exit(1)
    elif http_status == 0:
        print("FAIL: could not connect to server (connection refused or network error)")
        sys.exit(1)
    elif http_status and http_status != 200:
        err_msg = "\n".join(non_sse_lines) or f"HTTP {http_status}"
        print(f"FAIL: server returned HTTP {http_status}: {err_msg}")
        sys.exit(1)
    elif non_sse_lines:
        err_msg = "\n".join(non_sse_lines)
        print(f"FAIL: server returned non-SSE response: {err_msg}")
        sys.exit(1)
    else:
        print("FAIL: no events received from server")
        sys.exit(1)

# Check (a): accepted comes first and done last
ev_first = events[0]["event"]
if ev_first != "accepted":
    print(f"FAIL: first event was {ev_first!r}, expected \x27accepted\x27")
    sys.exit(1)

ev_last = events[-1]["event"]
if ev_last != "done":
    if ev_last == "failed":
        err = events[-1]["data"].get("error", "unknown error") if isinstance(events[-1]["data"], dict) else "failed"
        print(f"FAIL: stream failed: {err}")
        sys.exit(1)
    print(f"FAIL: last event was {ev_last!r}, expected \x27done\x27")
    sys.exit(1)

if no_stages:
    print("PASS")
    sys.exit(0)

# Check (b): stages contact, feeling, recognition, impulsion and registration
# each appear exactly once, in that order (investigation and determining may be measured: false)
required = ["contact", "feeling", "recognition", "impulsion", "registration"]
valid_stages = ["contact", "feeling", "recognition", "investigation", "determining", "impulsion", "registration"]

stream_stages = []
for ev in events:
    if ev["event"] == "stage":
        st = ev["data"].get("stage") if isinstance(ev["data"], dict) else None
        if st:
            stream_stages.append(st)
    elif isinstance(ev["data"], dict) and "stage" in ev["data"]:
        stream_stages.append(ev["data"]["stage"])

for req in required:
    c = stream_stages.count(req)
    if c == 0:
        print(f"FAIL: missing required stage {req!r} in stream (stages found: {stream_stages})")
        sys.exit(1)
    if c > 1:
        print(f"FAIL: stage {req!r} appeared {c} times in stream, expected exactly once")
        sys.exit(1)

req_indices = [stream_stages.index(r) for r in required]
if req_indices != sorted(req_indices):
    print(f"FAIL: required stages appeared out of order: {stream_stages}")
    sys.exit(1)

for opt in ["investigation", "determining"]:
    c = stream_stages.count(opt)
    if c > 1:
        print(f"FAIL: stage {opt!r} appeared {c} times in stream, expected at most once")
        sys.exit(1)

rec_idx = stream_stages.index("recognition")
imp_idx = stream_stages.index("impulsion")

if "investigation" in stream_stages:
    inv_idx = stream_stages.index("investigation")
    if not (rec_idx < inv_idx < imp_idx):
        print(f"FAIL: investigation stage appeared out of order: {stream_stages}")
        sys.exit(1)

if "determining" in stream_stages:
    det_idx = stream_stages.index("determining")
    if not (rec_idx < det_idx < imp_idx):
        print(f"FAIL: determining stage appeared out of order: {stream_stages}")
        sys.exit(1)
    if "investigation" in stream_stages and stream_stages.index("investigation") > det_idx:
        print(f"FAIL: determining stage appeared before investigation: {stream_stages}")
        sys.exit(1)

for s in stream_stages:
    if s not in valid_stages:
        print(f"FAIL: unexpected stage {s!r} in stream")
        sys.exit(1)

# Check (c): the done turn carries a vithi array with the same stages
done_data = events[-1]["data"]
if not isinstance(done_data, dict):
    print("FAIL: done event data is not an object")
    sys.exit(1)

turn = done_data.get("turn")
if isinstance(turn, dict) and "vithi" in turn:
    vithi = turn.get("vithi")
elif "vithi" in done_data:
    vithi = done_data.get("vithi")
else:
    print("FAIL: done turn missing \x27vithi\x27 array")
    sys.exit(1)

if not isinstance(vithi, list):
    vtype = type(vithi).__name__
    print(f"FAIL: \x27vithi\x27 in done turn is not an array (type: {vtype})")
    sys.exit(1)

vithi_stages = []
for item in vithi:
    if isinstance(item, dict):
        s = item.get("stage")
        if not s:
            print(f"FAIL: entry in done turn vithi missing \x27stage\x27 field: {item}")
            sys.exit(1)
        vithi_stages.append(s)
    elif isinstance(item, str):
        vithi_stages.append(item)
    else:
        print(f"FAIL: invalid entry in done turn vithi array: {item}")
        sys.exit(1)

if vithi_stages != stream_stages:
    print(f"FAIL: done turn vithi stages {vithi_stages} do not match stream stages {stream_stages}")
    sys.exit(1)

print("PASS")
sys.exit(0)
' "$@"
