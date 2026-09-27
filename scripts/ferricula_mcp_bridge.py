#!/usr/bin/env python3
"""
Ferricula MCP stdio bridge (JSON-RPC 2.0 over stdin/stdout).

Exposes a running Ferricula agent runtime to stdio-only MCP clients. The
runtime already serves MCP over HTTP at POST /mcp; use this bridge only for
clients that cannot speak Streamable HTTP. It is persona-neutral: the agent's
name comes from the runtime's /status and /identity endpoints.

Configuration (environment only):
  FERRICULA_URL                   runtime base URL (default http://127.0.0.1:8875)
  FERRICULA_OPERATOR_TOKEN        operator bearer token, or
  FERRICULA_OPERATOR_TOKEN_FILE   path to a file containing it
  FERRICULA_CHAT_TIMEOUT          seconds to wait for /chat (default 300)

Tools:
  ferricula_status    GET  /status
  ferricula_identity  GET  /identity
  ferricula_recall        POST /memory/recall   (hybrid: memory metadata + verbatim document sections)
  ferricula_chat          POST /chat            (the runtime routes the model call; durable turns)
  ferricula_ingest        POST /documents       (text | url | local PDF/text path | pdf_base64)
  ferricula_documents     GET  /documents
  ferricula_read_section  GET  /documents/{doc_id}/sections/{index}

The v2 names steve_status / steve_identity / steve_recall / steve_chat are
accepted by tools/call as deprecated aliases but are not listed.
"""

import base64
import json
import os
import urllib.parse
import sys
import urllib.error
import urllib.request
import uuid

BASE_URL = (os.environ.get("FERRICULA_URL")
            or os.environ.get("FERRICULA_BASE_URL")
            or "http://127.0.0.1:8875").rstrip("/")
CHAT_TIMEOUT = float(os.environ.get("FERRICULA_CHAT_TIMEOUT", "300"))
PROTOCOL_VERSION = "2024-11-05"


def _load_token():
    token = os.environ.get("FERRICULA_OPERATOR_TOKEN", "").strip()
    if token:
        return token
    path = os.environ.get("FERRICULA_OPERATOR_TOKEN_FILE", "").strip()
    if path:
        try:
            with open(path, "r", encoding="utf-8") as f:
                return f.read().strip()
        except OSError as e:
            sys.stderr.write(f"ferricula bridge: cannot read token file: {e}\n")
    return ""


TOKEN = _load_token()

DEPRECATED_ALIASES = {
    "steve_status": "ferricula_status",
    "steve_identity": "ferricula_identity",
    "steve_recall": "ferricula_recall",
    "steve_chat": "ferricula_chat",
}


def http_request(path, method="GET", data=None, timeout=30):
    headers = {"Accept": "application/json"}
    if TOKEN:
        headers["Authorization"] = f"Bearer {TOKEN}"
    body = None
    if data is not None:
        body = json.dumps(data).encode("utf-8")
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(f"{BASE_URL}{path}", data=body, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return json.loads(resp.read().decode("utf-8")), False
    except urllib.error.HTTPError as e:
        try:
            detail = json.loads(e.read().decode("utf-8"))
        except Exception:
            detail = {"error": e.reason}
        detail.setdefault("http_status", e.code)
        return detail, True
    except Exception as e:  # connection refused, timeout, bad JSON
        return {"error": f"request to {path} failed: {e}"}, True


def tool_status(_args):
    return http_request("/status")


def tool_identity(_args):
    return http_request("/identity")


def tool_recall(args):
    query = str(args.get("query", "")).strip()
    if not query:
        return {"error": "query cannot be empty"}, True
    limit = args.get("limit", 10)
    try:
        limit = max(1, min(int(limit), 64))
    except (TypeError, ValueError):
        return {"error": "limit must be an integer"}, True
    return http_request("/memory/recall", "POST", {"query": query, "limit": limit})


def tool_chat(args):
    message = args.get("message")
    if not isinstance(message, str) or not message.strip():
        return {"error": "message is required"}, True
    conversation_id = args.get("conversation_id") or str(uuid.uuid4())
    try:
        conversation_id = str(uuid.UUID(str(conversation_id)))
    except ValueError:
        return {"error": "conversation_id must be a uuid"}, True
    turn, failed = http_request(
        "/chat",
        "POST",
        {
            "request_id": str(uuid.uuid4()),
            "conversation_id": conversation_id,
            "message": message,
            "reported_origin": "agent",
        },
        timeout=CHAT_TIMEOUT,
    )
    if failed:
        turn.setdefault("conversation_id", conversation_id)
        return turn, True
    return {
        "reply": turn.get("reply"),
        "status": turn.get("status"),
        "model": turn.get("model"),
        "error": turn.get("error"),
        "conversation_id": conversation_id,
    }, turn.get("status") != "completed" or not turn.get("reply")


MAX_DOCUMENT_BYTES = int(os.environ.get("FERRICULA_MAX_DOCUMENT_BYTES", str(32 * 1024 * 1024)))


def tool_ingest(args):
    note = args.get("note")
    sources = [k for k in ("text", "url", "path", "pdf_base64") if args.get(k)]
    if len(sources) != 1:
        return {"error": "provide exactly one of text, url, path, or pdf_base64"}, True
    kind = sources[0]
    if kind == "text":
        body = {"kind": "text", "text": str(args["text"])}
        if args.get("title"):
            body["title"] = str(args["title"])
    elif kind == "url":
        body = {"kind": "url", "url": str(args["url"])}
    elif kind == "pdf_base64":
        body = {"kind": "pdf", "name": str(args.get("name") or "document.pdf"),
                "base64": str(args["pdf_base64"])}
    else:
        path = os.path.expanduser(str(args["path"]))
        try:
            size = os.path.getsize(path)
            if size > MAX_DOCUMENT_BYTES:
                return {"error": f"{path} is {size} bytes; limit is {MAX_DOCUMENT_BYTES}"}, True
            with open(path, "rb") as f:
                data = f.read()
        except OSError as e:
            return {"error": f"cannot read {path}: {e}"}, True
        name = args.get("name") or os.path.basename(path)
        if data.startswith(b"%PDF"):
            # Encoded client-side: the runtime never reads the caller's filesystem.
            body = {"kind": "pdf", "name": str(name), "base64": base64.b64encode(data).decode("ascii")}
        else:
            try:
                text = data.decode("utf-8")
            except UnicodeDecodeError:
                return {"error": f"{path} is neither a PDF nor UTF-8 text"}, True
            body = {"kind": "text", "text": text, "title": str(args.get("title") or name)}
    if note:
        body["note"] = str(note)
    return http_request("/documents", "POST", body, timeout=CHAT_TIMEOUT)


def tool_documents(_args):
    return http_request("/documents")


def tool_read_section(args):
    doc_id = str(args.get("doc_id", "")).strip()
    if not doc_id:
        return {"error": "doc_id is required"}, True
    try:
        index = int(args.get("index"))
        if index < 0:
            raise ValueError
    except (TypeError, ValueError):
        return {"error": "index must be a non-negative integer"}, True
    return http_request(f"/documents/{urllib.parse.quote(doc_id, safe='')}/sections/{index}")


TOOLS = {
    "ferricula_status": (tool_status, {
        "description": "Read-only agent runtime status (identity, mode, memory counts). No memory text.",
        "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False},
    }),
    "ferricula_identity": (tool_identity, {
        "description": "Read-only identity inspection of the mounted memory (counts, keystones, graph).",
        "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False},
    }),
    "ferricula_recall": (tool_recall, {
        "description": "Read-only hybrid recall: recovered-memory and experience hits (ids, tags, refs) "
                       "fused by rank with document sections carrying verbatim text and a "
                       "[doc id\u00a7index p.page] citation.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Non-empty lexical query"},
                "limit": {"type": "integer", "minimum": 1, "maximum": 64, "default": 10},
            },
            "required": ["query"],
            "additionalProperties": False,
        },
    }),
    "ferricula_chat": (tool_chat, {
        "description": "Send one operator message to the agent via the runtime's POST /chat. "
                       "Returns the reply and conversation_id; turns are stored by the runtime.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "message": {"type": "string", "description": "Message, 1 to 8192 UTF-8 bytes"},
                "conversation_id": {"type": "string", "description": "Optional UUID to continue a conversation"},
            },
            "required": ["message"],
            "additionalProperties": False,
        },
    }),
    "ferricula_ingest": (tool_ingest, {
        "description": "Hand the agent a document to read: inline text/markdown, a URL (web page or PDF), "
                       "a local file path (PDF is base64-encoded client-side; other files are sent as UTF-8 text), "
                       "or pdf_base64. Stored verbatim and remembered as an 'I read X' experience.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "text": {"type": "string", "description": "Inline text or markdown"},
                "title": {"type": "string", "description": "Optional title for text"},
                "url": {"type": "string", "description": "http(s) URL of a web page or PDF"},
                "path": {"type": "string", "description": "Local path to a PDF or text/markdown file"},
                "pdf_base64": {"type": "string", "description": "Base64-encoded PDF bytes"},
                "name": {"type": "string", "description": "File name for a PDF"},
                "note": {"type": "string", "description": "Optional: why you are giving the agent this"},
            },
            "additionalProperties": False,
        },
    }),
    "ferricula_documents": (tool_documents, {
        "description": "List documents the agent has read (doc_id, title, origin, pages, sections).",
        "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False},
    }),
    "ferricula_read_section": (tool_read_section, {
        "description": "Return one document section's exact text and citation handle.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "doc_id": {"type": "string"},
                "index": {"type": "integer", "minimum": 0},
            },
            "required": ["doc_id", "index"],
            "additionalProperties": False,
        },
    }),
}


def _server_title():
    status, failed = http_request("/status", timeout=5)
    if not failed and isinstance(status, dict) and status.get("identity"):
        return str(status["identity"])
    return "Ferricula"


def handle_rpc(request):
    req_id = request.get("id")
    method = request.get("method")
    params = request.get("params") or {}

    def ok(result):
        return {"jsonrpc": "2.0", "id": req_id, "result": result}

    def err(code, message):
        return {"jsonrpc": "2.0", "id": req_id, "error": {"code": code, "message": message}}

    if method == "initialize":
        return ok({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "ferricula", "title": _server_title(), "version": "3.0.0-alpha.0"},
        })
    if method and method.startswith("notifications/"):
        return None
    if method == "ping":
        return ok({})
    if method == "tools/list":
        return ok({"tools": [dict(name=name, **spec) for name, (_, spec) in TOOLS.items()]})
    if method == "tools/call":
        name = params.get("name") or ""
        name = DEPRECATED_ALIASES.get(name, name)
        entry = TOOLS.get(name)
        if entry is None:
            return err(-32602, f"Unknown tool: {name}")
        args = params.get("arguments") or {}
        if not isinstance(args, dict):
            return err(-32602, "arguments must be an object")
        payload, is_error = entry[0](args)
        return ok({"content": [{"type": "text", "text": json.dumps(payload, indent=2)}], "isError": bool(is_error)})
    return err(-32601, f"Method {method} not found")


def run_stdio():
    for stream in (sys.stdin, sys.stdout):
        if hasattr(stream, "reconfigure"):
            try:
                stream.reconfigure(line_buffering=True)
            except Exception:
                pass
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            resp = handle_rpc(json.loads(line))
        except Exception as e:
            resp = {"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": str(e)}}
        if resp is not None:
            sys.stdout.write(json.dumps(resp) + "\n")
            sys.stdout.flush()


if __name__ == "__main__":
    run_stdio()
