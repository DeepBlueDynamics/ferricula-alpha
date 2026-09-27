#!/usr/bin/env python3
"""
Smoke-test scripts/ferricula_mcp_bridge.py against a running runtime.

Uses the same environment as the bridge (FERRICULA_URL,
FERRICULA_OPERATOR_TOKEN or FERRICULA_OPERATOR_TOKEN_FILE). Optional:
  VERIFY_RECALL_QUERY   query for the recall step (default "memory")
  VERIFY_CHAT_MESSAGE   message for the chat step (skipped when unset)
  VERIFY_OUT            path to write a JSON summary

Exit code 0 when every executed step passes.
"""

import json
import os
import subprocess
import sys
import time

BRIDGE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "ferricula_mcp_bridge.py")


def main():
    proc = subprocess.Popen(
        [sys.executable, "-u", BRIDGE],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, env=os.environ.copy(), bufsize=1,
    )
    results = {}
    next_id = [0]

    def rpc(method, params=None):
        next_id[0] += 1
        proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": next_id[0], "method": method,
                                     "params": params or {}}) + "\n")
        proc.stdin.flush()
        line = proc.stdout.readline()
        if not line:
            raise RuntimeError(f"bridge closed stdout: {proc.stderr.read()}")
        return json.loads(line)

    def call(name, arguments):
        res = rpc("tools/call", {"name": name, "arguments": arguments})
        result = res.get("result", {})
        text = (result.get("content") or [{}])[0].get("text", "{}")
        return result.get("isError", True), json.loads(text)

    def step(label, fn):
        t0 = time.time()
        try:
            passed, detail = fn()
        except Exception as e:
            passed, detail = False, {"exception": str(e)}
        results[label] = {"status": "PASS" if passed else "FAIL",
                          "elapsed_s": round(time.time() - t0, 3), "detail": detail}
        print(f"{label}: {results[label]['status']} ({results[label]['elapsed_s']}s)")

    try:
        step("initialize", lambda: (lambda r: (r.get("result", {}).get("serverInfo", {}).get("name") == "ferricula", r))(
            rpc("initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                               "clientInfo": {"name": "verifier", "version": "1"}})))
        expected = {"ferricula_status", "ferricula_identity", "ferricula_recall", "ferricula_chat"}
        step("tools_list", lambda: (lambda names: (set(names) == expected, names))(
            [t["name"] for t in rpc("tools/list").get("result", {}).get("tools", [])]))
        step("ferricula_status", lambda: (lambda r: (not r[0], r[1]))(call("ferricula_status", {})))
        query = os.environ.get("VERIFY_RECALL_QUERY", "memory")
        step("ferricula_recall", lambda: (lambda r: (not r[0] and "hits" in r[1],
                                                     {"hits": len(r[1].get("hits", []))}))(
            call("ferricula_recall", {"query": query, "limit": 3})))
        message = os.environ.get("VERIFY_CHAT_MESSAGE")
        if message:
            step("ferricula_chat", lambda: (lambda r: (not r[0] and bool(r[1].get("reply")), r[1]))(
                call("ferricula_chat", {"message": message})))
    finally:
        proc.stdin.close()
        proc.terminate()
        proc.wait(timeout=5)

    out = os.environ.get("VERIFY_OUT")
    if out:
        with open(out, "w", encoding="utf-8") as f:
            json.dump(results, f, indent=2)
        print(f"summary written to {out}")
    return 0 if all(r["status"] == "PASS" for r in results.values()) else 1


if __name__ == "__main__":
    sys.exit(main())
