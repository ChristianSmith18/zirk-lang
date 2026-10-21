#!/usr/bin/env python3
"""Exercise public HTTP/3 WebSocket APIs with Zirk and independent aioquic peers."""
from __future__ import annotations

import asyncio
import gzip
import os
from pathlib import Path
import secrets
import socket
import ssl
import subprocess
import sys
import tempfile
import time

from aioquic.asyncio import serve
from aioquic.asyncio.client import connect
from aioquic.asyncio.protocol import QuicConnectionProtocol
from aioquic.h3.connection import H3Connection, H3_ALPN
from aioquic.h3.events import DataReceived, HeadersReceived
from aioquic.quic.configuration import QuicConfiguration
from cryptography import x509
from cryptography.hazmat.primitives import serialization

ROOT = Path(__file__).resolve().parents[1]
RUNTIME_TESTDATA = ROOT / "crates" / "zirk-runtime" / "testdata"


def run(command: list[str], *, cwd: Path, env: dict[str, str], input_text: str | None = None) -> str:
    result = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        input=input_text,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=180,
        check=False,
    )
    if result.returncode:
        raise RuntimeError(f"command failed ({result.returncode}): {' '.join(command)}\n{result.stdout}")
    return result.stdout


def byte_array(data: bytes) -> str:
    return "[" + ", ".join(f"{byte} as Byte" for byte in data) + "]"


def isolated_env(home: Path) -> dict[str, str]:
    env = os.environ.copy()
    env["HOME"] = str(home)
    env["USERPROFILE"] = str(home)
    env["APPDATA"] = str(home / "AppData" / "Roaming")
    env["LOCALAPPDATA"] = str(home / "AppData" / "Local")
    env["NO_COLOR"] = "1"
    return env


def make_server(project: Path, port: int, certificate: bytes, private_key: bytes) -> None:
    (project / "src").mkdir(parents=True, exist_ok=True)
    (project / ".zkinit").write_text(
        "project \"http3-websocket-server-test\"\n"
        "kind application\nentry \"src/main.zrk\"\n"
        "permissions {\nnetwork {\n"
        f'udp_listen: {{ addresses: ["127.0.0.1:{port}"]; during: runtime; }}\n'
        "}\n}\n",
        encoding="utf-8",
    )
    source = f'''import {{ HTTPServer, HTTPWebSocket, HTTPWebSocketMessage }} from std.http;
import {{ stdout }} from std.io;
fn handle(socket: HTTPWebSocket): Void {{
    stdout.println("handler-run");
    _ = socket.receive();
    _ = socket.send(HTTPWebSocketMessage.Text("zirk-http3-echo"));
    _ = socket.close();
}}
fn main(): Void {{
    mut certificates: Array<Array<Byte>> = [{byte_array(certificate)}];
    mut private_key: Array<Byte> = {byte_array(private_key)};
    mut origins: List<String> = List();
    origins.add("https://app.example");
    mut protocols: List<String> = List();
    protocols.add("echo.v1");
    match HTTPServer.serve_http3_websocket_once(
        "udp://127.0.0.1:{port}", certificates, private_key, "/socket",
        origins, protocols, 8, handle,
    ) {{
        Error(error) => stdout.println(error.to_string());
        Ok(_) => stdout.println("server-done");
    }}
}}
'''
    (project / "src" / "main.zrk").write_text(source, encoding="utf-8")


def make_client(project: Path, port: int, root_certificate: bytes) -> None:
    (project / "src").mkdir(parents=True)
    (project / ".zkinit").write_text(
        "project \"http3-websocket-client-test\"\n"
        "kind application\nentry \"src/main.zrk\"\n"
        "permissions {\nnetwork {\n"
        f'connect: {{ origins: ["https://127.0.0.1:{port}"]; during: runtime; }}\n'
        "}\n}\n",
        encoding="utf-8",
    )
    source = f'''import {{ HTTPWebSocket, HTTPWebSocketMessage, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    mut roots: Array<Array<Byte>> = [{byte_array(root_certificate)}];
    mut protocols: List<String> = List();
    protocols.add("echo.v1");
    match URL.parse("https://127.0.0.1:{port}/socket") {{
        Error(_) => stdout.println("url-error");
        Ok(url) => match HTTPWebSocket.connect_http3_with_trust_roots(
            url, "https://app.example", protocols, roots,
        ) {{
            Error(error) => stdout.println(error.to_string());
            Ok(socket) => {{
                _ = socket.send(HTTPWebSocketMessage.Text("hello"));
                match socket.receive() {{
                    Error(error) => stdout.println(error.to_string());
                    Ok(message) => match message {{
                        HTTPWebSocketMessage.Text(text) => stdout.println(text);
                        _ => stdout.println("unexpected-message");
                    }}
                }}
                match socket.close() {{
                    Error(error) => stdout.println(error.to_string());
                    Ok(_) => stdout.println("client-closed");
                }}
            }}
        }}
    }}
}}
'''
    (project / "src" / "main.zrk").write_text(source, encoding="utf-8")


def make_http3_client(project: Path, port: int, root_certificate: bytes) -> None:
    (project / "src").mkdir(parents=True)
    endpoint = f"https://127.0.0.1:{port}"
    (project / ".zkinit").write_text(
        "project \"http3-independent-peer-client\"\n"
        "kind application\nentry \"src/main.zrk\"\n"
        "permissions {\nnetwork {\n"
        f'connect: {{ origins: ["{endpoint}"]; during: runtime; }}\n'
        "}\n}\n",
        encoding="utf-8",
    )
    query_body = byte_array(b'{"q":"zirk"}')
    source = f'''import {{ HTTPClient, HTTPClientOptions, HTTPHeaders, HTTPMethod, HTTPProtocolPreference, HTTPProtocolVersion, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    mut roots: Array<Array<Byte>> = [{byte_array(root_certificate)}];
    match HTTPClientOptions.create_with_protocol(HTTPProtocolPreference.Auto, 5000, 10000, 10000, 1) {{
        Error(_) => stdout.println("options-error");
        Ok(options) => match HTTPClient.create_with_options_and_trust_roots(options, roots) {{
            Error(_) => stdout.println("client-error");
            Ok(client) => match URL.parse("{endpoint}/get") {{
                Error(_) => stdout.println("url-error");
                Ok(get_url) => {{
                    mut empty: Array<Byte> = [];
                    match client.request(HTTPMethod.Get, get_url, empty, HTTPHeaders.create()) {{
                        Error(error) => stdout.println(error.to_string());
                        Ok(response) => {{
                            if response.protocol() == HTTPProtocolVersion.HTTP3 {{ stdout.println("protocol-http3"); }}
                            stdout.println(response.status_code().to_string());
                            match response.text() {{ Ok(text) => stdout.println(text); Error(_) => stdout.println("get-body-error"); }}
                        }}
                    }}
                    match URL.parse("{endpoint}/query") {{
                        Error(_) => stdout.println("query-url-error");
                        Ok(query_url) => {{
                            mut headers = HTTPHeaders.create();
                            _ = headers.add("content-type", "application/query+json");
                            mut body: Array<Byte> = {query_body};
                            match client.request(HTTPMethod.Query, query_url, body, headers) {{
                                Error(error) => stdout.println(error.to_string());
                                Ok(response) => {{
                                    if response.protocol() == HTTPProtocolVersion.HTTP3 {{ stdout.println("protocol-http3"); }}
                                    stdout.println(response.status_code().to_string());
                                    match response.text() {{ Ok(text) => stdout.println(text); Error(_) => stdout.println("query-body-error"); }}
                                }}
                            }}
                        }}
                    }}
                    _ = client.close();
                    stdout.println("http-client-complete");
                }}
            }}
        }}
    }}
}}
'''
    (project / "src" / "main.zrk").write_text(source, encoding="utf-8")


def make_auto_fallback_client(project: Path, port: int, root_certificate: bytes) -> None:
    (project / "src").mkdir(parents=True)
    endpoint = f"https://127.0.0.1:{port}"
    (project / ".zkinit").write_text(
        'project "http-auto-fallback-client-test"\n'
        "kind application\nentry \"src/main.zrk\"\n"
        "permissions {\nnetwork {\n"
        f'connect: {{ origins: ["{endpoint}"]; during: runtime; }}\n'
        "}\n}\n",
        encoding="utf-8",
    )
    query_body = byte_array(b'{"fallback":true}')
    source = f'''import {{ HTTPClient, HTTPClientOptions, HTTPHeaders, HTTPMethod, HTTPProtocolPreference, HTTPProtocolVersion, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    mut roots: Array<Array<Byte>> = [{byte_array(root_certificate)}];
    match HTTPClientOptions.create_with_protocol(HTTPProtocolPreference.Auto, 5000, 10000, 10000, 1) {{
        Error(_) => stdout.println("options-error");
        Ok(options) => match HTTPClient.create_with_options_and_trust_roots(options, roots) {{
            Error(_) => stdout.println("client-error");
            Ok(client) => match URL.parse("{endpoint}/query") {{
                Error(_) => stdout.println("url-error");
                Ok(url) => {{
                    mut headers = HTTPHeaders.create();
                    _ = headers.add("content-type", "application/query+json");
                    mut body: Array<Byte> = {query_body};
                    match client.request(HTTPMethod.Query, url, body, headers) {{
                        Error(error) => stdout.println(error.to_string());
                        Ok(response) => {{
                            if response.protocol() == HTTPProtocolVersion.HTTP1 {{ stdout.println("fallback-http1"); }}
                            stdout.println(response.status_code().to_string());
                            match response.text() {{ Ok(text) => stdout.println(text); Error(_) => stdout.println("fallback-body-error"); }}
                        }}
                    }}
                    _ = client.close();
                    stdout.println("fallback-complete");
                }}
            }}
        }}
    }}
}}
'''
    (project / "src" / "main.zrk").write_text(source, encoding="utf-8")


def approve(zirk: Path, project: Path, env: dict[str, str]) -> None:
    output = run(
        [str(zirk), "permissions", "approve", "."],
        cwd=project,
        env=env,
        input_text="approve\n",
    )
    if "Approval saved" not in output:
        raise RuntimeError(f"permission approval did not complete:\n{output}")


def wait_until_bound(server: subprocess.Popen[str], port: int) -> None:
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        if server.poll() is not None:
            stdout, _ = server.communicate()
            raise RuntimeError(f"Zirk HTTP/3 server exited before binding:\n{stdout}")
        probe = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        try:
            probe.bind(("127.0.0.1", port))
        except OSError:
            return
        finally:
            probe.close()
        time.sleep(0.05)
    raise TimeoutError("the Zirk HTTP/3 server did not bind its UDP endpoint")


class IndependentH3Client(QuicConnectionProtocol):
    """Small aioquic peer that checks RFC 9220 against the public Zirk server."""

    def __init__(self, *args, **kwargs) -> None:
        super().__init__(*args, **kwargs)
        self.http = H3Connection(self._quic)
        self.events: asyncio.Queue[object] = asyncio.Queue()

    def quic_event_received(self, event) -> None:
        for http_event in self.http.handle_event(event):
            self.events.put_nowait(http_event)


class IndependentH3Server(QuicConnectionProtocol):
    """aioquic HTTP/3 server used as the independent Zirk client peer."""

    last_handshake_validated = False
    last_client_message: bytes | None = None
    last_close_received = False
    last_query_body: bytes | None = None
    last_query_content_type: bytes | None = None
    observed_user_agents: list[bytes | None] = []

    def __init__(self, *args, **kwargs) -> None:
        super().__init__(*args, **kwargs)
        self.http = H3Connection(self._quic)
        self.websockets: set[int] = set()
        self.buffers: dict[int, bytearray] = {}
        self.requests: dict[int, dict[bytes, bytes]] = {}

    def quic_event_received(self, event) -> None:
        for http_event in self.http.handle_event(event):
            if isinstance(http_event, HeadersReceived):
                headers = dict(http_event.headers)
                if headers.get(b":method") != b"CONNECT":
                    self.requests[http_event.stream_id] = headers
                    self.buffers[http_event.stream_id] = bytearray()
                    if http_event.stream_ended:
                        self.respond_http_request(http_event.stream_id)
                    continue
                valid = (
                    headers.get(b":method") == b"CONNECT"
                    and headers.get(b":protocol") == b"websocket"
                    and headers.get(b":path") == b"/socket"
                    and headers.get(b"origin") == b"https://app.example"
                    and headers.get(b"sec-websocket-version") == b"13"
                    and headers.get(b"sec-websocket-protocol") == b"echo.v1"
                )
                if not valid:
                    self.http.send_headers(
                        http_event.stream_id,
                        [(b":status", b"403")],
                        end_stream=True,
                    )
                    self.transmit()
                    continue
                self.websockets.add(http_event.stream_id)
                self.buffers[http_event.stream_id] = bytearray()
                type(self).last_handshake_validated = True
                self.http.send_headers(
                    http_event.stream_id,
                    [(b":status", b"200"), (b"sec-websocket-protocol", b"echo.v1")],
                )
                self.transmit()
            elif isinstance(http_event, DataReceived) and http_event.stream_id in self.websockets:
                buffer = self.buffers[http_event.stream_id]
                buffer.extend(http_event.data)
                while frame := take_websocket_frame(buffer):
                    opcode, payload = frame
                    if opcode == 0x1:
                        type(self).last_client_message = payload
                        if payload != b"hello":
                            raise RuntimeError(f"unexpected Zirk WebSocket message: {payload!r}")
                        self.http.send_data(
                            http_event.stream_id,
                            websocket_frame(0x1, b"aioquic-http3-echo", masked=False),
                            False,
                        )
                        self.transmit()
                    elif opcode == 0x8:
                        type(self).last_close_received = True
                        self.http.send_data(
                            http_event.stream_id,
                            websocket_frame(0x8, payload[:125], masked=False),
                            True,
                        )
                        self.transmit()
                    else:
                        raise RuntimeError(f"unexpected Zirk WebSocket opcode: {opcode}")
            elif isinstance(http_event, DataReceived) and http_event.stream_id in self.requests:
                self.buffers[http_event.stream_id].extend(http_event.data)
                if http_event.stream_ended:
                    self.respond_http_request(http_event.stream_id)

    def respond_http_request(self, stream_id: int) -> None:
        headers = self.requests.pop(stream_id)
        type(self).observed_user_agents.append(headers.get(b"user-agent"))
        method = headers.get(b":method", b"")
        path = headers.get(b":path", b"")
        body = bytes(self.buffers.pop(stream_id))
        if method == b"QUERY":
            type(self).last_query_body = body
            type(self).last_query_content_type = headers.get(b"content-type")
            if path != b"/query" or type(self).last_query_content_type != b"application/query+json":
                status = b"400"
                response_body = b"bad-query"
            else:
                status = b"200"
                response_body = b"aioquic-query:" + body
        elif method == b"GET" and path == b"/get":
            status = b"200"
            response_body = gzip.compress(b"aioquic-get")
        else:
            status = b"405"
            response_body = b"method-not-allowed"
        response_headers = [
            (b":status", status),
            (b"content-type", b"text/plain"),
            (b"content-length", str(len(response_body)).encode()),
        ]
        if method == b"GET" and path == b"/get":
            response_headers.append((b"content-encoding", b"gzip"))
        self.http.send_headers(stream_id, response_headers)
        self.http.send_data(stream_id, response_body, True)
        self.transmit()


def websocket_frame(opcode: int, payload: bytes, *, masked: bool = True) -> bytes:
    if len(payload) >= 126:
        raise ValueError("test WebSocket payload must fit in a short frame")
    if not masked:
        return bytes((0x80 | opcode, len(payload))) + payload
    mask = secrets.token_bytes(4)
    masked_payload = bytes(byte ^ mask[index % 4] for index, byte in enumerate(payload))
    return bytes((0x80 | opcode, 0x80 | len(payload))) + mask + masked_payload


def take_websocket_frame(buffer: bytearray) -> tuple[int, bytes] | None:
    if len(buffer) < 2:
        return None
    second = buffer[1]
    length = second & 0x7F
    offset = 2
    if length == 126:
        if len(buffer) < 4:
            return None
        length = int.from_bytes(buffer[2:4], "big")
        offset = 4
    elif length == 127:
        if len(buffer) < 10:
            return None
        length = int.from_bytes(buffer[2:10], "big")
        offset = 10
    mask = buffer[1] & 0x80
    opcode = buffer[0] & 0x0F
    if mask:
        offset += 4
    if len(buffer) < offset + length:
        return None
    payload = bytes(buffer[offset : offset + length])
    if mask:
        key = buffer[offset - 4 : offset]
        payload = bytes(byte ^ key[index % 4] for index, byte in enumerate(payload))
    del buffer[: offset + length]
    return opcode, payload


async def aioquic_websocket_round_trip(port: int, ca_file: Path) -> None:
    configuration = QuicConfiguration(
        is_client=True,
        alpn_protocols=H3_ALPN,
        cafile=str(ca_file),
        server_name="127.0.0.1",
        idle_timeout=10,
    )
    async with connect(
        "127.0.0.1",
        port,
        configuration=configuration,
        create_protocol=IndependentH3Client,
    ) as protocol:
        assert isinstance(protocol, IndependentH3Client)
        stream_id = protocol._quic.get_next_available_stream_id()
        protocol.http.send_headers(
            stream_id,
            [
                (b":method", b"CONNECT"),
                (b":protocol", b"websocket"),
                (b":scheme", b"https"),
                (b":authority", f"127.0.0.1:{port}".encode()),
                (b":path", b"/socket"),
                (b"origin", b"https://app.example"),
                (b"sec-websocket-version", b"13"),
                (b"sec-websocket-protocol", b"echo.v1"),
            ],
        )
        protocol.http.send_data(stream_id, websocket_frame(0x1, b"aioquic-peer"), False)
        protocol.transmit()

        status: bytes | None = None
        selected_protocol: bytes | None = None
        received = bytearray()
        echo_received = False
        deadline = asyncio.get_running_loop().time() + 8
        while asyncio.get_running_loop().time() < deadline:
            event = await asyncio.wait_for(protocol.events.get(), timeout=2)
            if isinstance(event, HeadersReceived):
                headers = dict(event.headers)
                status = headers.get(b":status")
                selected_protocol = headers.get(b"sec-websocket-protocol")
                continue
            if isinstance(event, DataReceived):
                received.extend(event.data)
                while frame := take_websocket_frame(received):
                    opcode, payload = frame
                    if opcode == 0x1:
                        if payload != b"zirk-http3-echo":
                            raise RuntimeError(f"unexpected Zirk WebSocket text: {payload!r}")
                        echo_received = True
                    elif opcode == 0x8:
                        protocol.http.send_data(stream_id, websocket_frame(0x8, payload[:125]), False)
                        protocol.transmit()
                        if (
                            status != b"200"
                            or selected_protocol != b"echo.v1"
                            or not echo_received
                        ):
                            raise RuntimeError(
                                f"unexpected extended CONNECT response: {status!r}, {selected_protocol!r}"
                            )
                        return
                    else:
                        raise RuntimeError(f"unexpected WebSocket opcode: {opcode}")
        raise TimeoutError("aioquic did not receive the Zirk WebSocket echo and close")


async def aioquic_rejected_origin(port: int, ca_file: Path) -> None:
    configuration = QuicConfiguration(
        is_client=True,
        alpn_protocols=H3_ALPN,
        cafile=str(ca_file),
        server_name="127.0.0.1",
        idle_timeout=10,
    )
    async with connect(
        "127.0.0.1",
        port,
        configuration=configuration,
        create_protocol=IndependentH3Client,
    ) as protocol:
        assert isinstance(protocol, IndependentH3Client)
        stream_id = protocol._quic.get_next_available_stream_id()
        protocol.http.send_headers(
            stream_id,
            [
                (b":method", b"CONNECT"),
                (b":protocol", b"websocket"),
                (b":scheme", b"https"),
                (b":authority", f"127.0.0.1:{port}".encode()),
                (b":path", b"/socket"),
                (b"origin", b"https://unapproved.example"),
                (b"sec-websocket-version", b"13"),
                (b"sec-websocket-protocol", b"echo.v1"),
            ],
        )
        protocol.transmit()
        deadline = asyncio.get_running_loop().time() + 8
        while asyncio.get_running_loop().time() < deadline:
            event = await asyncio.wait_for(protocol.events.get(), timeout=2)
            if isinstance(event, HeadersReceived):
                status = dict(event.headers).get(b":status")
                if status != b"403":
                    raise RuntimeError(f"unapproved Origin received status {status!r}, expected 403")
                return
        raise TimeoutError("aioquic did not receive an Origin rejection")


def run_aioquic_peer(zirk: Path, project: Path, env: dict[str, str], port: int,
                     root_certificate: bytes) -> None:
    certificate = x509.load_der_x509_certificate(root_certificate)
    ca_file = project.parent / "aioquic-root.pem"
    ca_file.write_bytes(certificate.public_bytes(serialization.Encoding.PEM))
    server = subprocess.Popen(
        [str(zirk), "run", "."],
        cwd=project,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    try:
        wait_until_bound(server, port)
        asyncio.run(aioquic_websocket_round_trip(port, ca_file))
        server_output, _ = server.communicate(timeout=30)
        if server.returncode or "server-done" not in server_output:
            raise RuntimeError(f"Zirk server did not complete cleanly:\n{server_output}")
        if "handler-run" not in server_output:
            raise RuntimeError(f"Zirk did not dispatch the approved WebSocket handler:\n{server_output}")
    except Exception as error:
        if server.poll() is None:
            server.kill()
        server_output, _ = server.communicate()
        raise RuntimeError(f"{error}\nserver output before failure:\n{server_output}") from error


def run_aioquic_origin_rejection(
    zirk: Path, project: Path, env: dict[str, str], port: int, ca_file: Path
) -> None:
    server = subprocess.Popen(
        [str(zirk), "run", "."],
        cwd=project,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    try:
        wait_until_bound(server, port)
        asyncio.run(aioquic_rejected_origin(port, ca_file))
    except Exception as error:
        if server.poll() is None:
            server.kill()
        server_output, _ = server.communicate()
        raise RuntimeError(f"{error}\nserver output before failure:\n{server_output}") from error
    if server.poll() is None:
        server.kill()
    server_output, _ = server.communicate()
    if "handler-run" in server_output:
        raise RuntimeError(f"Zirk dispatched a handler for an unapproved Origin:\n{server_output}")


async def aioquic_server_with_zirk_client(
    zirk: Path,
    websocket_project: Path,
    websocket_env: dict[str, str],
    http_project: Path,
    http_env: dict[str, str],
    fallback_project: Path,
    fallback_home: Path,
    port: int,
    certificate_der: bytes,
    private_key_der: bytes,
) -> None:
    IndependentH3Server.last_handshake_validated = False
    IndependentH3Server.last_client_message = None
    IndependentH3Server.last_close_received = False
    IndependentH3Server.last_query_body = None
    IndependentH3Server.last_query_content_type = None
    IndependentH3Server.observed_user_agents = []
    configuration = QuicConfiguration(
        is_client=False,
        alpn_protocols=H3_ALPN,
        certificate=x509.load_der_x509_certificate(certificate_der),
        private_key=serialization.load_der_private_key(private_key_der, password=None),
        idle_timeout=10,
    )
    server = await serve(
        "127.0.0.1",
        port,
        configuration=configuration,
        create_protocol=IndependentH3Server,
    )
    try:
        process = await asyncio.create_subprocess_exec(
            str(zirk),
            "run",
            ".",
            cwd=websocket_project,
            env=websocket_env,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.STDOUT,
        )
        stdout, _ = await asyncio.wait_for(process.communicate(), timeout=60)
        output = stdout.decode(errors="replace")
        if process.returncode or "aioquic-http3-echo" not in output or "client-closed" not in output:
            raise RuntimeError(f"Zirk client did not complete the aioquic exchange:\n{output}")
        if (
            not IndependentH3Server.last_handshake_validated
            or IndependentH3Server.last_client_message != b"hello"
            or not IndependentH3Server.last_close_received
        ):
            raise RuntimeError(
                "aioquic did not validate the RFC 9220 handshake, message, and close"
            )

        http_process = await asyncio.create_subprocess_exec(
            str(zirk),
            "run",
            ".",
            cwd=http_project,
            env=http_env,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.STDOUT,
        )
        http_stdout, _ = await asyncio.wait_for(http_process.communicate(), timeout=60)
        http_output = http_stdout.decode(errors="replace")
        expected_query = 'aioquic-query:{"q":"zirk"}'
        if (
            http_process.returncode
            or http_output.count("protocol-http3") != 2
            or "aioquic-get" not in http_output
            or expected_query not in http_output
            or "200" not in http_output
        ):
            raise RuntimeError(f"Zirk HTTP/3 GET/QUERY exchange failed:\n{http_output}")
        if (
            IndependentH3Server.last_query_body != b'{"q":"zirk"}'
            or IndependentH3Server.last_query_content_type != b"application/query+json"
        ):
            raise RuntimeError("aioquic did not receive the exact QUERY body and content type")
        expected_user_agent = b"Zirk-http-client/0.0.0"
        if IndependentH3Server.observed_user_agents != [expected_user_agent, expected_user_agent]:
            raise RuntimeError(
                "HTTP/3 requests did not use the versioned Zirk User-Agent: "
                f"{IndependentH3Server.observed_user_agents!r}"
            )
    finally:
        server.close()
    await auto_fallback_to_http1(
        zirk,
        fallback_project,
        fallback_home,
        port,
        certificate_der,
        private_key_der,
    )


async def auto_fallback_to_http1(
    zirk: Path,
    project: Path,
    home: Path,
    port: int,
    certificate: bytes,
    private_key: bytes,
) -> None:
    certificate_path = home / "server-cert.pem"
    key_path = home / "server-key.pem"
    certificate_path.write_bytes(
        x509.load_der_x509_certificate(certificate).public_bytes(serialization.Encoding.PEM)
    )
    private_key_path = serialization.load_der_private_key(private_key, password=None)
    key_path.write_bytes(
        private_key_path.private_bytes(
            serialization.Encoding.PEM,
            serialization.PrivateFormat.PKCS8,
            serialization.NoEncryption(),
        )
    )
    tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    tls.load_cert_chain(certificate_path, key_path)
    tls.set_alpn_protocols(["http/1.1"])
    requests: list[tuple[bytes, bytes]] = []

    async def handle(reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
        try:
            head = await reader.readuntil(b"\r\n\r\n")
            headers = head.split(b"\r\n")
            content_length = next(
                (
                    int(line.split(b":", 1)[1].strip())
                    for line in headers[1:]
                    if line.lower().startswith(b"content-length:")
                ),
                0,
            )
            body = await reader.readexactly(content_length) if content_length else b""
            requests.append((headers[0], body))
            writer.write(
                b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\nfallback-ok"
            )
            await writer.drain()
        finally:
            writer.close()
            await writer.wait_closed()

    server = await asyncio.start_server(handle, "127.0.0.1", port, ssl=tls)
    try:
        output = await asyncio.to_thread(
            run,
            [str(zirk), "run", "."],
            cwd=project,
            env=isolated_env(home),
        )
        if "fallback-http1" not in output or "fallback-ok" not in output:
            raise RuntimeError(f"Auto did not fall back to HTTP/1.1:\n{output}")
        if requests != [(b"QUERY /query HTTP/1.1", b'{"fallback":true}')]:
            raise RuntimeError(f"HTTP/1.1 fallback received an unexpected request: {requests!r}")
    finally:
        server.close()
        await server.wait_closed()


def main() -> int:
    run(
        ["cargo", "build", "-p", "zirk-cli", "-p", "zirk-runtime", "--features", "zirk-runtime/http3"],
        cwd=ROOT,
        env=os.environ.copy(),
    )
    exe = "zirk.exe" if os.name == "nt" else "zirk"
    zirk = ROOT / "target" / "debug" / exe
    if not zirk.is_file():
        raise FileNotFoundError(f"compiler binary was not produced: {zirk}")

    certificate = (RUNTIME_TESTDATA / "tls-loopback-cert.der").read_bytes()
    private_key = (RUNTIME_TESTDATA / "tls-loopback-key.der").read_bytes()
    root_certificate = (RUNTIME_TESTDATA / "tls-loopback-root.der").read_bytes()

    with tempfile.TemporaryDirectory(prefix="zirk-http3-websocket-") as temporary:
        temporary_root = Path(temporary)
        server_project = temporary_root / "server"
        client_project = temporary_root / "client"
        http_client_project = temporary_root / "http-client"
        fallback_project = temporary_root / "http-fallback-client"
        server_home = temporary_root / "server-home"
        client_home = temporary_root / "client-home"
        http_client_home = temporary_root / "http-client-home"
        fallback_home = temporary_root / "http-fallback-home"
        server_home.mkdir()
        client_home.mkdir()
        http_client_home.mkdir()
        fallback_home.mkdir()

        reservation = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        reservation.bind(("127.0.0.1", 0))
        port = reservation.getsockname()[1]
        reservation.close()

        make_server(server_project, port, certificate, private_key)
        make_client(client_project, port, root_certificate)
        make_http3_client(http_client_project, port, root_certificate)
        make_auto_fallback_client(fallback_project, port, root_certificate)
        server_env = isolated_env(server_home)
        client_env = isolated_env(client_home)
        http_client_env = isolated_env(http_client_home)
        fallback_env = isolated_env(fallback_home)
        approve(zirk, server_project, server_env)
        approve(zirk, client_project, client_env)
        approve(zirk, http_client_project, http_client_env)
        approve(zirk, fallback_project, fallback_env)

        server = subprocess.Popen(
            [str(zirk), "run", "."],
            cwd=server_project,
            env=server_env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        try:
            wait_until_bound(server, port)
            client_output = run(
                [str(zirk), "run", "."],
                cwd=client_project,
                env=client_env,
            )
            if "zirk-http3-echo" not in client_output or "client-closed" not in client_output:
                raise RuntimeError(f"Zirk client did not complete the echo and close:\n{client_output}")
            server_output, _ = server.communicate(timeout=30)
            if server.returncode or "server-done" not in server_output:
                raise RuntimeError(f"Zirk server did not complete cleanly:\n{server_output}")
        except Exception as error:
            if server.poll() is None:
                server.kill()
            server_output, _ = server.communicate()
            raise RuntimeError(f"{error}\nserver output before failure:\n{server_output}") from error

        make_server(server_project, port, certificate, private_key)
        run_aioquic_peer(
            zirk,
            server_project,
            server_env,
            port,
            root_certificate,
        )
        run_aioquic_origin_rejection(
            zirk,
            server_project,
            server_env,
            port,
            server_project.parent / "aioquic-root.pem",
        )

        asyncio.run(
            aioquic_server_with_zirk_client(
                zirk,
                client_project,
                client_env,
                http_client_project,
                http_client_env,
                fallback_project,
                fallback_home,
                port,
                certificate,
                private_key,
            )
        )

    print("HTTP/3 interoperability, Origin rejection, and Auto fallback passed with independent peers.")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(error, file=sys.stderr)
        raise SystemExit(1)
