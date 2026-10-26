#!/usr/bin/env python3
"""Measure initialize and didOpen-to-diagnostics using the local Zirk LSP."""
import json
import pathlib
import subprocess
import sys
import time
import argparse

parser = argparse.ArgumentParser()
parser.add_argument("source", type=pathlib.Path)
parser.add_argument("--changes", type=int, default=0, help="measure this many full-document didChange edits")
parser.add_argument("--server", type=pathlib.Path, default=pathlib.Path("target/debug/zirk-lsp"))
args = parser.parse_args()
source_path = args.source.resolve()
source = source_path.read_text()
proc = subprocess.Popen([str(args.server)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)

def send(message):
    payload = json.dumps(message, separators=(",", ":")).encode()
    proc.stdin.write(f"Content-Length: {len(payload)}\r\n\r\n".encode() + payload)
    proc.stdin.flush()

def read_message():
    headers = {}
    while True:
        line = proc.stdout.readline()
        if not line:
            raise RuntimeError("LSP closed stdout")
        if line in (b"\r\n", b"\n"):
            break
        key, value = line.decode().split(":", 1)
        headers[key.lower()] = value.strip()
    return json.loads(proc.stdout.read(int(headers["content-length"])))

started = time.perf_counter()
send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"processId": None, "rootUri": source_path.parent.as_uri(), "capabilities": {}}})
while read_message().get("id") != 1:
    pass
initialized = time.perf_counter()
send({"jsonrpc": "2.0", "method": "initialized", "params": {}})
send({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": source_path.as_uri(), "languageId": "zirk", "version": 1, "text": source}}})
while True:
    response = read_message()
    if (
        response.get("method") == "textDocument/publishDiagnostics"
        and response.get("params", {}).get("uri") == source_path.as_uri()
        and response.get("params", {}).get("version") == 1
    ):
        break
analyzed = time.perf_counter()
change_times = []
for version in range(2, args.changes + 2):
    source += f"\n// incremental benchmark edit {version}\n"
    change_started = time.perf_counter()
    send({
        "jsonrpc": "2.0",
        "method": "textDocument/didChange",
        "params": {
            "textDocument": {"uri": source_path.as_uri(), "version": version},
            "contentChanges": [{"text": source}],
        },
    })
    while True:
        response = read_message()
        if (
            response.get("method") == "textDocument/publishDiagnostics"
            and response.get("params", {}).get("uri") == source_path.as_uri()
            and response.get("params", {}).get("version") == version
        ):
            break
    change_times.append(time.perf_counter() - change_started)
send({"jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": None})
while read_message().get("id") != 2:
    pass
send({"jsonrpc": "2.0", "method": "exit", "params": None})
proc.stdin.close()
try:
    proc.wait(timeout=2)
except subprocess.TimeoutExpired:
    proc.terminate()
    proc.wait(timeout=2)
print(f"initialize_seconds={initialized-started:.6f}")
print(f"didOpen_to_diagnostics_seconds={analyzed-initialized:.6f}")
print(f"diagnostics_uri={response.get('params', {}).get('uri', '')}")
diagnostics = response.get("params", {}).get("diagnostics", [])
print(f"diagnostics_count={len(diagnostics)}")
print(
    "diagnostic_codes="
    + "/".join(sorted({item.get("code", "?") for item in diagnostics}))
)
print(
    "diagnostic_messages="
    + json.dumps([item.get("message", "") for item in diagnostics], ensure_ascii=False)
)
for version, elapsed in enumerate(change_times, start=2):
    print(f"didChange_{version}_to_diagnostics_seconds={elapsed:.6f}")
