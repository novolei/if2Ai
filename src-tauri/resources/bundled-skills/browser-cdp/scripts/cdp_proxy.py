#!/usr/bin/env python3
"""
CDP Proxy — 常驻进程，复用单一 WebSocket 连接到 Chrome。

**解决的核心问题**：Chrome 136+ 的 ``chrome://inspect`` WS-only 模式下，
每个新的 WebSocket 连接都会触发一次 "要允许远程调试吗？" 弹窗。
CDP Proxy 保持**唯一一个** WS 连接到 Chrome，并向下游脚本暴露
标准的 HTTP + WS CDP 接口，从而让用户只需点击**一次** "允许"。

Architecture::

    Script-A ──┐                           ┌── Chrome CDP (9222)
    Script-B ──┤── CDP Proxy (9223) ───────┤   (WS-only mode)
    Script-C ──┘   本地 HTTP + WS 代理     └── 用户只弹一次窗

**功能**：

- 维持到 Chrome 的单一 WS 连接，自动重连
- 暴露标准 CDP HTTP 端点 (``/json/version``, ``/json/list``, ``/json/new``, etc.)
- 多路复用下游 WS 连接，转发 CDP 命令/事件
- 通过 PID 文件实现单例管理（防止重复启动）
- 支持优雅停止

Usage:
    # 启动 proxy（后台常驻）
    python cdp_proxy.py start --chrome-port 9222 --proxy-port 9223

    # 查询状态
    python cdp_proxy.py status

    # 停止
    python cdp_proxy.py stop

    # 或在代码中使用
    from cdp_proxy import CDPProxy, ensure_proxy_running
    proxy_url = ensure_proxy_running(chrome_port=9222)
"""
from __future__ import annotations

import os as _os, sys as _sys
_sys.path.insert(0, _os.path.dirname(_os.path.abspath(__file__)))
import _encoding_fix  # noqa: F401

import argparse
import atexit
import json
import os
import secrets
import signal
import socket
import struct
import sys
import tempfile
import threading
import time
import base64
import hashlib
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.request import urlopen, Request
from urllib.error import URLError


# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

DEFAULT_CHROME_PORT = 9222
DEFAULT_PROXY_PORT = 9223

# Proxy state file location — use user-specific directory to prevent cross-user attacks
import getpass
PROXY_STATE_DIR = os.path.join(tempfile.gettempdir(), f'cdp-proxy-{getpass.getuser()}')
PROXY_PID_FILE = os.path.join(PROXY_STATE_DIR, 'proxy.pid')
PROXY_STATE_FILE = os.path.join(PROXY_STATE_DIR, 'proxy.json')

# Reconnect settings
RECONNECT_INTERVAL_S = 2.0
RECONNECT_MAX_RETRIES = 30  # 60 seconds total

# WS frame constants
WS_FIN_TEXT = 0x81
WS_FIN_CLOSE = 0x88
WS_FIN_PING = 0x89
WS_FIN_PONG = 0x8A


# ---------------------------------------------------------------------------
# Minimal WebSocket implementation (no external deps for proxy)
# ---------------------------------------------------------------------------

def _ws_handshake_request(host: str, port: int, path: str) -> tuple:
    """Perform WS client handshake, return (socket, response_headers)."""
    ws_key = base64.b64encode(os.urandom(16)).decode()
    handshake = (
        f'GET {path} HTTP/1.1\r\n'
        f'Host: {host}:{port}\r\n'
        'Upgrade: websocket\r\n'
        'Connection: Upgrade\r\n'
        f'Sec-WebSocket-Key: {ws_key}\r\n'
        'Sec-WebSocket-Version: 13\r\n'
        '\r\n'
    )
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.settimeout(120)  # long timeout for user Allow dialog
    sock.connect((host, port))
    sock.sendall(handshake.encode())

    # Read response headers
    buf = b''
    while b'\r\n\r\n' not in buf:
        chunk = sock.recv(4096)
        if not chunk:
            sock.close()
            raise ConnectionError("Connection closed during handshake")
        buf += chunk

    header_text = buf.split(b'\r\n\r\n')[0].decode('utf-8', errors='replace')
    if '101' not in header_text:
        sock.close()
        raise ConnectionError(f"WS handshake failed: {header_text[:200]}")

    # Return socket and any remaining data after headers
    remainder = buf.split(b'\r\n\r\n', 1)[1]
    return sock, remainder


def _ws_send_frame(sock: socket.socket, payload: bytes, opcode: int = 0x01, masked: bool = True):
    """Send a WebSocket frame."""
    frame = bytearray()
    frame.append(0x80 | opcode)  # FIN + opcode

    mask_bit = 0x80 if masked else 0x00
    length = len(payload)
    if length < 126:
        frame.append(mask_bit | length)
    elif length < 65536:
        frame.append(mask_bit | 126)
        frame.extend(struct.pack('>H', length))
    else:
        frame.append(mask_bit | 127)
        frame.extend(struct.pack('>Q', length))

    if masked:
        mask_key = os.urandom(4)
        frame.extend(mask_key)
        frame.extend(bytearray(b ^ mask_key[i % 4] for i, b in enumerate(payload)))
    else:
        frame.extend(payload)

    sock.sendall(bytes(frame))


def _ws_recv_frame(sock: socket.socket, buf: bytearray) -> tuple:
    """
    Read one WS frame from sock + buf.
    Returns (opcode, payload_bytes, remaining_buf) or raises.
    """
    def _ensure(n):
        nonlocal buf
        while len(buf) < n:
            chunk = sock.recv(65536)
            if not chunk:
                raise ConnectionError("WS connection closed")
            buf.extend(chunk)

    _ensure(2)
    b0, b1 = buf[0], buf[1]
    opcode = b0 & 0x0F
    is_masked = (b1 & 0x80) != 0
    length = b1 & 0x7F
    offset = 2

    if length == 126:
        _ensure(offset + 2)
        length = struct.unpack('>H', bytes(buf[offset:offset+2]))[0]
        offset += 2
    elif length == 127:
        _ensure(offset + 8)
        length = struct.unpack('>Q', bytes(buf[offset:offset+8]))[0]
        offset += 8

    if is_masked:
        _ensure(offset + 4)
        mask = bytes(buf[offset:offset+4])
        offset += 4

    _ensure(offset + length)
    data = bytes(buf[offset:offset+length])

    if is_masked:
        data = bytes(b ^ mask[i % 4] for i, b in enumerate(data))

    remaining = bytearray(buf[offset+length:])
    return opcode, data, remaining


# ---------------------------------------------------------------------------
# Chrome upstream connection
# ---------------------------------------------------------------------------

class ChromeUpstream:
    """
    Manages the single WS connection to Chrome's CDP endpoint.
    Thread-safe: multiple downstream clients share this connection.
    """

    def __init__(self, host: str, port: int):
        self.host = host
        self.port = port
        self._sock = None
        self._send_lock = threading.Lock()   # protects socket writes
        self._state_lock = threading.Lock()  # protects socket/buf state
        self._recv_buf = bytearray()
        self._closed = False
        self._recv_thread = None
        self._on_message = None  # callback(str) for messages not matched by _pending
        self._on_disconnect = None  # callback() when upstream WS disconnects
        self._version_info = None  # cached version info from Browser.getVersion

        # Thread-safe pending requests: msg_id -> (Event, result_holder_list)
        self._pending_lock = threading.Lock()
        self._pending: dict[int, tuple[threading.Event, list]] = {}

    def connect(self, timeout: float = 120.0) -> bool:
        """
        Connect to Chrome CDP WS endpoint.
        Returns True on success, False on failure.
        """
        with self._state_lock:
            if self._sock:
                try:
                    self._sock.close()
                except OSError:
                    pass
                self._sock = None

        try:
            sock, remainder = _ws_handshake_request(
                self.host, self.port, '/devtools/browser'
            )
            with self._state_lock:
                self._sock = sock
                self._recv_buf = bytearray(remainder)
                self._closed = False

            # Start receive thread
            self._recv_thread = threading.Thread(
                target=self._recv_loop, daemon=True, name='chrome-upstream-recv'
            )
            self._recv_thread.start()

            # Fetch version info
            self._fetch_version_info()
            return True

        except (OSError, ConnectionError) as e:
            _log(f"Failed to connect to Chrome at {self.host}:{self.port}: {e}")
            return False

    def adopt_socket(self, sock: 'socket.socket') -> bool:
        """
        Adopt an already-authenticated WS socket to Chrome.

        This allows reusing a socket that was used in
        ``_trigger_and_wait_cdp_auth()`` to avoid triggering another
        "Allow remote debugging?" dialog in Chrome's WS-only mode.

        The socket must already have completed the WebSocket handshake
        and be in a ready state for sending/receiving WS frames.

        Returns True on success, False on failure.
        """
        with self._state_lock:
            if self._sock:
                try:
                    self._sock.close()
                except OSError:
                    pass

            self._sock = sock
            self._recv_buf = bytearray()
            self._closed = False

        # Start receive thread
        self._recv_thread = threading.Thread(
            target=self._recv_loop, daemon=True, name='chrome-upstream-recv'
        )
        self._recv_thread.start()

        # Fetch version info
        self._fetch_version_info()
        return self._version_info is not None

    def _fetch_version_info(self):
        """Send Browser.getVersion and cache the result."""
        try:
            result = self.send_and_wait(
                {'id': -1, 'method': 'Browser.getVersion'},
                timeout=5.0,
            )
            if result and 'result' in result:
                self._version_info = result['result']
        except Exception:
            pass

    @property
    def connected(self) -> bool:
        return self._sock is not None and not self._closed

    @property
    def version_info(self) -> dict:
        return self._version_info or {}

    def send(self, data: bytes):
        """Send raw WS text frame to Chrome."""
        with self._send_lock:
            with self._state_lock:
                sock = self._sock
                closed = self._closed
            if not sock or closed:
                raise ConnectionError("Not connected to Chrome")
            _ws_send_frame(sock, data, opcode=0x01, masked=True)

    def send_and_wait(self, msg: dict, timeout: float = 30.0) -> dict:
        """Send a message and wait for the response with matching id.

        Thread-safe: uses a per-request pending dict instead of replacing
        the global on_message callback.  This allows concurrent
        send_and_wait calls and does not interfere with downstream
        message routing.
        """
        msg_id = msg.get('id', -1)
        event = threading.Event()
        result_holder = [None]

        with self._pending_lock:
            self._pending[msg_id] = (event, result_holder)

        try:
            self.send(json.dumps(msg).encode())
            event.wait(timeout=timeout)
            return result_holder[0]
        finally:
            with self._pending_lock:
                self._pending.pop(msg_id, None)

    def set_on_message(self, callback):
        """Set callback for incoming WS messages from Chrome."""
        self._on_message = callback

    def set_on_disconnect(self, callback):
        """Set callback invoked when upstream WS connection is lost."""
        self._on_disconnect = callback

    def close(self):
        self._closed = True
        with self._state_lock:
            if self._sock:
                try:
                    self._sock.close()
                except OSError:
                    pass
                self._sock = None

    def _dispatch_message(self, text: str):
        """Route an incoming message: first check pending requests, then on_message."""
        try:
            data = json.loads(text)
        except (json.JSONDecodeError, ValueError):
            if self._on_message:
                self._on_message(text)
            return

        msg_id = data.get('id')
        if msg_id is not None:
            with self._pending_lock:
                pending = self._pending.get(msg_id)
            if pending:
                event, result_holder = pending
                result_holder[0] = data
                event.set()
                return

        # Not a pending internal request — forward to proxy router
        if self._on_message:
            self._on_message(text)

    def _recv_loop(self):
        while not self._closed:
            try:
                with self._state_lock:
                    sock = self._sock
                    buf = self._recv_buf
                if not sock:
                    break

                sock.settimeout(1.0)
                try:
                    chunk = sock.recv(65536)
                except socket.timeout:
                    continue
                if not chunk:
                    break

                buf.extend(chunk)

                # Parse all complete frames
                while True:
                    try:
                        opcode, payload, remaining = _ws_recv_frame(sock, buf)
                        with self._state_lock:
                            self._recv_buf = remaining
                        buf = remaining

                        if opcode == 0x01:  # Text
                            text = payload.decode('utf-8', errors='replace')
                            try:
                                self._dispatch_message(text)
                            except Exception:
                                pass
                        elif opcode == 0x08:  # Close
                            self._closed = True
                            return
                        elif opcode == 0x09:  # Ping
                            with self._send_lock:
                                with self._state_lock:
                                    s = self._sock
                                if s:
                                    _ws_send_frame(s, payload, opcode=0x0A, masked=True)
                    except (ConnectionError, struct.error):
                        break
                    except (TimeoutError, socket.timeout):
                        # _ws_recv_frame's _ensure() may call sock.recv()
                        # which times out when the buffer has no complete frame.
                        # This is normal — break inner loop to wait for more data.
                        break

            except (OSError, ConnectionError):
                break

        self._closed = True
        # Notify listener that upstream WS is disconnected
        if self._on_disconnect:
            try:
                self._on_disconnect()
            except Exception:
                pass


# ---------------------------------------------------------------------------
# Downstream client (script → proxy WS connection)
# ---------------------------------------------------------------------------

class DownstreamClient:
    """Represents one WS connection from a script to the proxy."""

    def __init__(self, sock: socket.socket, addr, client_id: int):
        self.sock = sock
        self.addr = addr
        self.client_id = client_id
        self._recv_buf = bytearray()
        self._closed = False
        self._lock = threading.Lock()

    def send_text(self, text: str):
        """Send a WS text frame to the downstream client (unmasked, server→client)."""
        with self._lock:
            if self._closed:
                return
            try:
                _ws_send_frame(self.sock, text.encode(), opcode=0x01, masked=False)
            except (OSError, BrokenPipeError):
                self._closed = True

    def close(self):
        self._closed = True
        try:
            self.sock.close()
        except OSError:
            pass


# ---------------------------------------------------------------------------
# CDP Proxy Server
# ---------------------------------------------------------------------------

class CDPProxy:
    """
    CDP Proxy server.

    - Maintains a single WS connection to Chrome (upstream).
    - Accepts multiple WS connections from scripts (downstream).
    - Multiplexes CDP commands/events between them.
    - Provides standard CDP HTTP endpoints for discovery.
    """

    def __init__(self, chrome_host: str = '127.0.0.1', chrome_port: int = DEFAULT_CHROME_PORT,
                 proxy_port: int = DEFAULT_PROXY_PORT):
        # Generate a random token for authenticating downstream clients
        self._auth_token = secrets.token_urlsafe(32)
        self.chrome_host = chrome_host
        self.chrome_port = chrome_port
        self.proxy_port = proxy_port

        self._upstream = ChromeUpstream(chrome_host, chrome_port)
        self._downstream_clients: dict[int, DownstreamClient] = {}
        self._client_counter = 0
        self._lock = threading.Lock()
        self._running = False

        # Message ID remapping: each downstream client's msg IDs get
        # remapped to globally unique IDs to avoid collisions.
        self._id_offset = 0
        self._id_map: dict[int, tuple[int, int]] = {}  # global_id -> (client_id, original_id)

        # HTTP server for /json/* endpoints
        self._http_server = None

        # Session tracking: session_id -> client_id
        self._session_map: dict[str, int] = {}

        # Auto-exit timer: when upstream disconnects, wait this long before stopping
        self._disconnect_timer: threading.Timer | None = None
        self._disconnect_timeout_s = 60  # seconds

    def start(self, block: bool = True, auth_sock: 'socket.socket | None' = None):
        """Start the proxy. If block=True, runs until interrupted.

        Args:
            block: If True, block until interrupted.
            auth_sock: An already-authenticated WS socket to Chrome.
                If provided, the proxy will adopt this socket instead of
                opening a new connection (avoids triggering another auth
                dialog in Chrome's WS-only mode).
        """
        _log(f"Starting CDP Proxy: Chrome={self.chrome_host}:{self.chrome_port}, "
             f"Proxy=127.0.0.1:{self.proxy_port}")

        self._running = True

        # Connect to Chrome (reuse auth_sock if provided)
        if not self._connect_upstream(auth_sock=auth_sock):
            _log("Failed to connect to Chrome. Will retry in background.")

        # Start HTTP server for /json/* endpoints
        self._start_http_server()

        # Start WS listener for downstream clients
        ws_thread = threading.Thread(
            target=self._ws_accept_loop, daemon=True, name='proxy-ws-accept'
        )
        ws_thread.start()

        # Write state file
        self._write_state()

        _log(f"CDP Proxy ready on port {self.proxy_port}")
        _log(f"  HTTP: http://127.0.0.1:{self.proxy_port}/json/version")
        _log(f"  WS:   ws://127.0.0.1:{self.proxy_port}/devtools/browser")

        if block:
            try:
                while self._running:
                    time.sleep(1)
            except KeyboardInterrupt:
                _log("Interrupted, stopping...")
            finally:
                self.stop()

    def stop(self):
        """Stop the proxy and clean up."""
        self._running = False

        # Cancel any pending auto-exit timer
        self._cancel_disconnect_timer()

        # Close all downstream clients
        with self._lock:
            for client in self._downstream_clients.values():
                client.close()
            self._downstream_clients.clear()

        # Close upstream
        self._upstream.close()

        # Stop HTTP server
        if self._http_server:
            self._http_server.shutdown()

        # Clean up state files
        self._cleanup_state()
        _log("CDP Proxy stopped.")

    def _connect_upstream(self, auth_sock: 'socket.socket | None' = None) -> bool:
        """Connect to Chrome, with retries.

        If auth_sock is provided, adopt it directly (no new connection,
        no auth dialog).
        """
        if auth_sock:
            if self._upstream.adopt_socket(auth_sock):
                self._upstream.set_on_message(self._on_upstream_message)
                self._upstream.set_on_disconnect(self._on_upstream_disconnect)
                self._cancel_disconnect_timer()
                _log("Adopted authenticated socket to Chrome CDP (no new dialog).")
                return True
            else:
                _log("Failed to adopt authenticated socket, falling back to new connection.")
                # Fall through to normal connect

        for attempt in range(RECONNECT_MAX_RETRIES):
            if not self._running:
                return False
            if self._upstream.connect():
                self._upstream.set_on_message(self._on_upstream_message)
                self._upstream.set_on_disconnect(self._on_upstream_disconnect)
                self._cancel_disconnect_timer()
                _log("Connected to Chrome CDP.")
                return True
            if attempt < RECONNECT_MAX_RETRIES - 1:
                time.sleep(RECONNECT_INTERVAL_S)
        return False

    def _on_upstream_message(self, text: str):
        """Handle a message from Chrome → route to appropriate downstream client."""
        try:
            msg = json.loads(text)
        except (json.JSONDecodeError, ValueError):
            return

        # Response to a command (has 'id')
        if 'id' in msg:
            global_id = msg['id']
            with self._lock:
                mapping = self._id_map.pop(global_id, None)
            if mapping:
                client_id, original_id = mapping
                msg['id'] = original_id  # Restore original ID
                with self._lock:
                    client = self._downstream_clients.get(client_id)
                if client:
                    client.send_text(json.dumps(msg))

                    # Track session attachments
                    result = msg.get('result', {})
                    if 'sessionId' in result:
                        session_id = result['sessionId']
                        with self._lock:
                            self._session_map[session_id] = client_id

        # Event (no 'id', has 'method')
        elif 'method' in msg:
            session_id = msg.get('sessionId')
            if session_id:
                # Route to the client that owns this session
                with self._lock:
                    client_id = self._session_map.get(session_id)
                    client = self._downstream_clients.get(client_id) if client_id else None
                if client:
                    client.send_text(text)
            else:
                # Browser-level event → broadcast to all clients
                with self._lock:
                    clients = list(self._downstream_clients.values())
                for client in clients:
                    client.send_text(text)

    def _ws_accept_loop(self):
        """Listen for incoming WS connections from scripts."""
        server_sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        server_sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        server_sock.settimeout(1.0)
        server_sock.bind(('127.0.0.1', self.proxy_port))
        server_sock.listen(16)

        while self._running:
            try:
                client_sock, addr = server_sock.accept()
            except socket.timeout:
                continue
            except OSError:
                break

            threading.Thread(
                target=self._handle_ws_upgrade,
                args=(client_sock, addr),
                daemon=True,
            ).start()

        server_sock.close()

    def _handle_ws_upgrade(self, sock: socket.socket, addr):
        """Handle WS upgrade handshake from a downstream client, then proxy."""
        try:
            sock.settimeout(10)
            buf = b''
            while b'\r\n\r\n' not in buf:
                chunk = sock.recv(4096)
                if not chunk:
                    sock.close()
                    return
                buf += chunk

            header_text = buf.split(b'\r\n\r\n')[0].decode('utf-8', errors='replace')

            # Check if this is an HTTP request (not WS upgrade)
            if 'upgrade: websocket' not in header_text.lower():
                self._handle_http_on_ws_port(sock, header_text)
                return

            # SECURITY: Validate Origin header — reject non-localhost origins
            # This prevents malicious web pages from connecting via JS WebSocket.
            for line in header_text.split('\r\n'):
                if line.lower().startswith('origin:'):
                    origin = line.split(':', 1)[1].strip().lower()
                    allowed_origins = {'http://127.0.0.1', 'http://localhost',
                                       'https://127.0.0.1', 'https://localhost'}
                    # Allow empty origin (Python CDP clients) and localhost origins only
                    if origin and not any(origin.startswith(ao) for ao in allowed_origins):
                        _log(f"Rejected WS connection from non-localhost origin: {origin}")
                        sock.close()
                        return
                    break

            # SECURITY: Validate auth token from query string
            first_line = header_text.split('\r\n')[0]
            request_path = first_line.split()[1] if len(first_line.split()) >= 2 else ''
            token_valid = False
            if '?token=' in request_path:
                from urllib.parse import urlparse, parse_qs
                parsed = urlparse(request_path)
                token_param = parse_qs(parsed.query).get('token', [None])[0]
                if token_param == self._auth_token:
                    token_valid = True
            if not token_valid:
                _log(f"Rejected WS connection from {addr}: invalid or missing token")
                reject_response = (
                    'HTTP/1.1 403 Forbidden\r\n'
                    'Content-Length: 0\r\n'
                    'Connection: close\r\n'
                    '\r\n'
                )
                try:
                    sock.sendall(reject_response.encode())
                except OSError:
                    pass
                sock.close()
                return

            # Parse Sec-WebSocket-Key
            ws_key = None
            for line in header_text.split('\r\n'):
                if line.lower().startswith('sec-websocket-key:'):
                    ws_key = line.split(':', 1)[1].strip()
                    break

            if not ws_key:
                sock.close()
                return

            # Compute accept key
            accept_key = base64.b64encode(
                hashlib.sha1((ws_key + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest()
            ).decode()

            # Send upgrade response
            response = (
                'HTTP/1.1 101 Switching Protocols\r\n'
                'Upgrade: websocket\r\n'
                'Connection: Upgrade\r\n'
                f'Sec-WebSocket-Accept: {accept_key}\r\n'
                '\r\n'
            )
            sock.sendall(response.encode())

            # Register downstream client
            with self._lock:
                self._client_counter += 1
                client_id = self._client_counter
            client = DownstreamClient(sock, addr, client_id)
            with self._lock:
                self._downstream_clients[client_id] = client

            _log(f"Downstream client #{client_id} connected from {addr}")

            # Proxy loop: read from downstream, forward to upstream
            recv_buf = bytearray()
            remainder = buf.split(b'\r\n\r\n', 1)[1]
            if remainder:
                recv_buf.extend(remainder)

            sock.settimeout(1.0)
            while self._running and not client._closed:
                try:
                    chunk = sock.recv(65536)
                except socket.timeout:
                    continue
                except OSError:
                    break
                if not chunk:
                    break

                recv_buf.extend(chunk)

                # Parse frames
                while True:
                    try:
                        opcode, payload, recv_buf = _ws_recv_frame(sock, recv_buf)

                        if opcode == 0x01:  # Text frame
                            self._forward_downstream_to_upstream(client_id, payload)
                        elif opcode == 0x08:  # Close
                            break
                        elif opcode == 0x09:  # Ping
                            _ws_send_frame(sock, payload, opcode=0x0A, masked=False)
                    except (ConnectionError, struct.error):
                        break
                    except (TimeoutError, socket.timeout):
                        # _ws_recv_frame's _ensure() may call sock.recv()
                        # which times out when buffer has no complete frame.
                        # Break inner loop to wait for more data.
                        break

        except (OSError, ConnectionError):
            pass
        finally:
            with self._lock:
                removed = self._downstream_clients.pop(client_id if 'client_id' in dir() else -1, None)
                # Clean up session mappings for this client
                if 'client_id' in dir():
                    to_remove = [sid for sid, cid in self._session_map.items() if cid == client_id]
                    for sid in to_remove:
                        del self._session_map[sid]
            if removed:
                _log(f"Downstream client #{removed.client_id} disconnected")
            try:
                sock.close()
            except OSError:
                pass

    def _forward_downstream_to_upstream(self, client_id: int, payload: bytes):
        """Forward a CDP message from a downstream client to Chrome."""
        try:
            text = payload.decode('utf-8', errors='replace')
            msg = json.loads(text)
        except (json.JSONDecodeError, ValueError):
            return

        # Remap message ID to avoid collisions
        if 'id' in msg:
            original_id = msg['id']
            with self._lock:
                self._id_offset += 1
                global_id = self._id_offset
                self._id_map[global_id] = (client_id, original_id)
            msg['id'] = global_id

        # Track session usage
        session_id = msg.get('sessionId')
        if session_id:
            with self._lock:
                self._session_map[session_id] = client_id

        # Forward to Chrome
        try:
            self._upstream.send(json.dumps(msg).encode())
        except ConnectionError:
            _log("Upstream disconnected, attempting reconnect...")
            threading.Thread(target=self._reconnect_upstream, daemon=True).start()

    def _reconnect_upstream(self):
        """Reconnect to Chrome in the background."""
        if self._connect_upstream():
            _log("Reconnected to Chrome.")
        else:
            _log("Failed to reconnect to Chrome after retries.")
            # Reconnect failed — start auto-exit timer
            self._start_disconnect_timer()

    def _on_upstream_disconnect(self):
        """Called when the upstream WS connection to Chrome is lost."""
        if not self._running:
            return
        _log(f"Upstream WS disconnected. Will auto-exit in {self._disconnect_timeout_s}s "
             "unless reconnected.")
        self._start_disconnect_timer()

    def _start_disconnect_timer(self):
        """Start (or restart) the auto-exit countdown."""
        self._cancel_disconnect_timer()
        timer = threading.Timer(self._disconnect_timeout_s, self._auto_exit_on_disconnect)
        timer.daemon = True
        timer.name = 'proxy-disconnect-timer'
        self._disconnect_timer = timer
        timer.start()

    def _cancel_disconnect_timer(self):
        """Cancel the auto-exit timer (e.g. on successful reconnect)."""
        timer = self._disconnect_timer
        if timer is not None:
            timer.cancel()
            self._disconnect_timer = None

    def _auto_exit_on_disconnect(self):
        """Auto-exit callback: upstream has been disconnected for too long."""
        if not self._running:
            return
        # Double-check: maybe we reconnected in the meantime
        if self._upstream.connected:
            _log("Upstream reconnected before auto-exit timer fired. Staying alive.")
            return
        _log(f"Upstream WS has been disconnected for {self._disconnect_timeout_s}s. "
             "Auto-stopping proxy.")
        self.stop()

    def _handle_http_on_ws_port(self, sock: socket.socket, header_text: str):
        """Handle plain HTTP requests on the WS port (for /json/* endpoints)."""
        try:
            first_line = header_text.split('\r\n')[0]
            parts = first_line.split()
            if len(parts) < 2:
                sock.close()
                return
            method, path = parts[0], parts[1]

            response_body = self._handle_json_endpoint(path)
            if response_body is not None:
                body_bytes = response_body.encode('utf-8')
                response = (
                    'HTTP/1.1 200 OK\r\n'
                    'Content-Type: application/json; charset=utf-8\r\n'
                    f'Content-Length: {len(body_bytes)}\r\n'
                    'Connection: close\r\n'
                    '\r\n'
                )
                sock.sendall(response.encode() + body_bytes)
            else:
                response = (
                    'HTTP/1.1 404 Not Found\r\n'
                    'Content-Length: 0\r\n'
                    'Connection: close\r\n'
                    '\r\n'
                )
                sock.sendall(response.encode())
        except (OSError, BrokenPipeError):
            pass
        finally:
            try:
                sock.close()
            except OSError:
                pass

    def _handle_json_endpoint(self, path: str) -> str | None:
        """Handle CDP HTTP API endpoints, return JSON string or None."""
        if path == '/json/version':
            info = dict(self._upstream.version_info)
            # Token is embedded in the webSocketDebuggerUrl, so only clients
            # that first discover via /json/version can connect.
            # This prevents blind WS connections from malicious web pages.
            info['webSocketDebuggerUrl'] = f'ws://127.0.0.1:{self.proxy_port}/devtools/browser?token={self._auth_token}'
            info['_proxy'] = True
            info['_proxy_port'] = self.proxy_port
            info['_chrome_port'] = self.chrome_port
            info['_upstream_connected'] = self._upstream.connected
            return json.dumps(info)

        elif path == '/json/list' or path == '/json':
            # Get targets via CDP command
            try:
                result = self._upstream.send_and_wait(
                    {'id': -2, 'method': 'Target.getTargets'},
                    timeout=5.0,
                )
                if result and 'result' in result:
                    targets = result['result'].get('targetInfos', [])
                    tab_list = []
                    for t in targets:
                        if t.get('type') == 'page':
                            tab_list.append({
                                'id': t.get('targetId', ''),
                                'type': t.get('type', ''),
                                'title': t.get('title', ''),
                                'url': t.get('url', ''),
                                'webSocketDebuggerUrl': f'ws://127.0.0.1:{self.proxy_port}/devtools/page/{t.get("targetId", "")}',
                            })
                    return json.dumps(tab_list)
            except Exception:
                pass
            return '[]'

        elif path.startswith('/json/new'):
            # Create new tab
            url = 'about:blank'
            if '?' in path:
                url = path.split('?', 1)[1]
                from urllib.parse import unquote
                url = unquote(url)
            try:
                result = self._upstream.send_and_wait(
                    {'id': -3, 'method': 'Target.createTarget', 'params': {'url': url}},
                    timeout=10.0,
                )
                if result and 'result' in result:
                    target_id = result['result'].get('targetId', '')
                    return json.dumps({
                        'id': target_id,
                        'type': 'page',
                        'title': '',
                        'url': url,
                        'webSocketDebuggerUrl': f'ws://127.0.0.1:{self.proxy_port}/devtools/page/{target_id}',
                    })
            except Exception:
                pass
            return None

        elif path.startswith('/json/close/'):
            target_id = path.split('/json/close/', 1)[1]
            try:
                self._upstream.send_and_wait(
                    {'id': -4, 'method': 'Target.closeTarget', 'params': {'targetId': target_id}},
                    timeout=5.0,
                )
                return '"Target is closing"'
            except Exception:
                return None

        elif path.startswith('/json/activate/'):
            target_id = path.split('/json/activate/', 1)[1]
            try:
                self._upstream.send_and_wait(
                    {'id': -5, 'method': 'Target.activateTarget', 'params': {'targetId': target_id}},
                    timeout=5.0,
                )
                return '"Target activated"'
            except Exception:
                return None

        return None

    def _start_http_server(self):
        """Start a simple HTTP server on a separate port for /json/* endpoints."""
        # We handle HTTP on the same port as WS (see _handle_http_on_ws_port)
        # so no separate HTTP server is needed.
        pass

    def _write_state(self):
        """Write proxy state to file for other processes to discover."""
        os.makedirs(PROXY_STATE_DIR, exist_ok=True)
        os.chmod(PROXY_STATE_DIR, 0o700)

        # Write PID file
        with open(PROXY_PID_FILE, 'w') as f:
            f.write(str(os.getpid()))

        # Write state file
        state = {
            'pid': os.getpid(),
            'proxy_port': self.proxy_port,
            'chrome_port': self.chrome_port,
            'chrome_host': self.chrome_host,
            'started_at': time.time(),
            'proxy_url': f'http://127.0.0.1:{self.proxy_port}',
            'auth_token': self._auth_token,
        }
        with open(PROXY_STATE_FILE, 'w') as f:
            json.dump(state, f)

    def _cleanup_state(self):
        """Remove state files."""
        for f in (PROXY_PID_FILE, PROXY_STATE_FILE):
            try:
                os.remove(f)
            except OSError:
                pass


# ---------------------------------------------------------------------------
# Proxy management utilities
# ---------------------------------------------------------------------------

def _log(msg: str):
    """Log to stderr with timestamp."""
    ts = time.strftime('%H:%M:%S')
    print(f"[cdp-proxy {ts}] {msg}", file=sys.stderr, flush=True)


def get_proxy_state() -> dict | None:
    """Read the proxy state file. Returns state dict or None."""
    if not os.path.isfile(PROXY_STATE_FILE):
        return None
    try:
        with open(PROXY_STATE_FILE, 'r') as f:
            state = json.load(f)
        # Verify the process is still alive
        pid = state.get('pid')
        if pid:
            try:
                os.kill(pid, 0)  # signal 0 = check existence
                return state
            except (OSError, ProcessLookupError):
                # Process is dead, clean up stale state
                _cleanup_stale_state()
                return None
        return None
    except (json.JSONDecodeError, OSError):
        return None


def _cleanup_stale_state():
    """Remove stale state files from a dead proxy."""
    for f in (PROXY_PID_FILE, PROXY_STATE_FILE):
        try:
            os.remove(f)
        except OSError:
            pass


def is_proxy_running() -> bool:
    """Check if a CDP proxy is currently running."""
    return get_proxy_state() is not None


def get_proxy_url() -> str | None:
    """Get the proxy URL if a proxy is running."""
    state = get_proxy_state()
    if state:
        return state.get('proxy_url')
    return None


def stop_proxy():
    """Stop the running proxy by sending SIGTERM."""
    state = get_proxy_state()
    if not state:
        _log("No proxy is running.")
        return False

    pid = state.get('pid')
    if pid:
        try:
            os.kill(pid, signal.SIGTERM)
            _log(f"Sent SIGTERM to proxy (PID {pid}).")
            # Wait briefly for cleanup
            for _ in range(10):
                try:
                    os.kill(pid, 0)
                    time.sleep(0.3)
                except (OSError, ProcessLookupError):
                    break
            _cleanup_stale_state()
            return True
        except (OSError, ProcessLookupError):
            _log(f"Proxy (PID {pid}) is not running. Cleaning up state.")
            _cleanup_stale_state()
            return False
    return False


def ensure_proxy_running(
    chrome_port: int = DEFAULT_CHROME_PORT,
    proxy_port: int = DEFAULT_PROXY_PORT,
    timeout: float = 30.0,
    auth_sock: 'socket.socket | None' = None,
) -> str:
    """
    Ensure a CDP proxy is running, starting one if needed.

    Returns the proxy URL (e.g. 'http://127.0.0.1:9223').

    This function is safe to call from any script — it will:
    1. Check if a proxy is already running → return its URL.
    2. If not, start a new proxy.
    3. Wait for the proxy to be ready before returning.

    Args:
        chrome_port: Chrome CDP port to proxy.
        proxy_port: Port for the proxy to listen on.
        timeout: Max seconds to wait for proxy readiness.
        auth_sock: An already-authenticated WS socket to Chrome.
            If provided, the proxy will be started via ``os.fork()``
            (macOS/Linux) so the child process inherits the socket and
            runs independently — surviving the parent script's exit.
            On Windows (no fork), falls back to an in-process daemon
            thread.  This avoids triggering a second auth dialog in
            Chrome's WS-only mode.
    """
    # Check if already running
    state = get_proxy_state()
    if state:
        existing_url = state.get('proxy_url')
        existing_port = state.get('proxy_port')
        # Quick health check: port listening + upstream connected
        if existing_port and _is_port_listening(existing_port):
            # Verify upstream is actually connected
            upstream_ok = False
            try:
                from urllib.request import urlopen, Request
                req = Request(
                    f'http://127.0.0.1:{existing_port}/json/version',
                    headers={'Accept': 'application/json'},
                )
                with urlopen(req, timeout=3) as resp:
                    info = json.loads(resp.read().decode('utf-8'))
                    upstream_ok = info.get('_upstream_connected', False)
            except Exception:
                pass

            if upstream_ok:
                # Proxy is healthy — close the auth_sock since it's not needed
                if auth_sock:
                    try:
                        auth_sock.close()
                    except OSError:
                        pass
                return existing_url
            else:
                # Proxy is running but upstream is dead — stop it so we
                # can start a fresh one with the new auth_sock.
                _log("Existing proxy has no upstream connection. Stopping it.")
                stop_proxy()
                time.sleep(0.5)  # Allow port to be released
        else:
            # Stale state
            _cleanup_stale_state()

    # --- Mode A: Forked proxy (when we have an auth_sock) ---
    # We use os.fork() so the child process inherits the authenticated
    # socket and can run independently after the parent script exits.
    # This is critical: a daemon-thread proxy would die when the parent
    # Python process finishes, forcing the user to re-authorize CDP on
    # the next invocation.
    if auth_sock:
        if sys.platform == 'win32':
            # Windows has no fork(); fall back to in-process daemon thread.
            # The proxy will die with the parent, but there's no alternative
            # for passing an already-authenticated socket on Windows.
            proxy = CDPProxy(
                chrome_port=chrome_port,
                proxy_port=proxy_port,
            )
            proxy_thread = threading.Thread(
                target=proxy.start,
                kwargs={'block': True, 'auth_sock': auth_sock},
                daemon=True,
                name='cdp-proxy-inprocess',
            )
            proxy_thread.start()

            proxy_url = f'http://127.0.0.1:{proxy_port}'
            deadline = time.time() + timeout
            while time.time() < deadline:
                if _is_port_listening(proxy_port):
                    time.sleep(0.3)
                    return proxy_url
                time.sleep(0.5)

            raise RuntimeError(f"In-process CDP Proxy did not become ready within {timeout}s")

        # macOS / Linux: fork a detached child process that inherits auth_sock
        pid = os.fork()
        if pid == 0:
            # ---- Child process: become the long-lived proxy ----
            try:
                # Detach from parent's process group so we survive parent exit
                os.setsid()

                # Redirect stdout/stderr to log file to avoid broken pipes
                os.makedirs(PROXY_STATE_DIR, exist_ok=True)
                log_path = os.path.join(PROXY_STATE_DIR, 'proxy.log')
                log_fd = os.open(log_path, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o644)
                os.dup2(log_fd, 1)  # stdout
                os.dup2(log_fd, 2)  # stderr
                os.close(log_fd)
                # Close stdin
                devnull = os.open(os.devnull, os.O_RDONLY)
                os.dup2(devnull, 0)
                os.close(devnull)

                # Handle termination gracefully
                proxy = CDPProxy(
                    chrome_port=chrome_port,
                    proxy_port=proxy_port,
                )

                def _handle_signal(signum, frame):
                    proxy.stop()
                    sys.exit(0)
                signal.signal(signal.SIGTERM, _handle_signal)
                signal.signal(signal.SIGINT, _handle_signal)

                proxy.start(block=True, auth_sock=auth_sock)
            except Exception:
                pass
            finally:
                os._exit(0)  # Ensure child never returns to caller
        else:
            # ---- Parent process: wait for proxy to become ready ----
            # Close auth_sock in parent — the child owns it now
            try:
                auth_sock.close()
            except OSError:
                pass

            proxy_url = f'http://127.0.0.1:{proxy_port}'
            deadline = time.time() + timeout
            while time.time() < deadline:
                if _is_port_listening(proxy_port):
                    time.sleep(0.3)
                    return proxy_url
                # Check if child died unexpectedly
                try:
                    wpid, status = os.waitpid(pid, os.WNOHANG)
                    if wpid != 0:
                        raise RuntimeError(
                            f"CDP Proxy child (PID {pid}) exited unexpectedly "
                            f"with status {status}"
                        )
                except ChildProcessError:
                    break
                time.sleep(0.5)

            raise RuntimeError(f"Forked CDP Proxy did not become ready within {timeout}s")

    # --- Mode B: Subprocess proxy (normal mode, no auth_sock) ---
    script_path = os.path.abspath(__file__)
    cmd = [
        sys.executable, script_path, 'start',
        '--chrome-port', str(chrome_port),
        '--proxy-port', str(proxy_port),
        '--daemon',
    ]

    # Use subprocess to start in background
    # Note: stderr goes to a log file (not PIPE) to prevent blocking
    # when the pipe buffer fills up after the parent exits.
    import subprocess
    log_path = os.path.join(PROXY_STATE_DIR, 'proxy.log')
    os.makedirs(PROXY_STATE_DIR, exist_ok=True)
    log_file = open(log_path, 'a')
    proc = subprocess.Popen(
        cmd,
        stdout=subprocess.DEVNULL,
        stderr=log_file,
        start_new_session=True,  # Detach from parent
    )

    # Wait for proxy to become ready
    proxy_url = f'http://127.0.0.1:{proxy_port}'
    deadline = time.time() + timeout
    while time.time() < deadline:
        if _is_port_listening(proxy_port):
            # Give it a moment to finish initialization
            time.sleep(0.3)
            return proxy_url
        # Check if process died
        if proc.poll() is not None:
            # Read last lines from log file for error info
            try:
                log_file.flush()
                with open(log_path, 'r') as lf:
                    log_content = lf.read()[-500:]
            except Exception:
                log_content = ''
            raise RuntimeError(
                f"CDP Proxy failed to start (exit code {proc.returncode}): {log_content}"
            )
        time.sleep(0.5)

    raise RuntimeError(f"CDP Proxy did not become ready within {timeout}s")


def _is_port_listening(port: int) -> bool:
    """Check if something is listening on localhost:port."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.settimeout(1)
        return s.connect_ex(('127.0.0.1', port)) == 0


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description='CDP Proxy — 复用单一 WS 连接，消除重复弹窗'
    )
    sub = parser.add_subparsers(dest='command')

    # start
    start_p = sub.add_parser('start', help='Start the proxy')
    start_p.add_argument('--chrome-port', type=int, default=DEFAULT_CHROME_PORT,
                         help=f'Chrome CDP port (default: {DEFAULT_CHROME_PORT})')
    start_p.add_argument('--proxy-port', type=int, default=DEFAULT_PROXY_PORT,
                         help=f'Proxy listen port (default: {DEFAULT_PROXY_PORT})')
    start_p.add_argument('--daemon', action='store_true',
                         help='Run as daemon (detach from terminal)')

    # status
    sub.add_parser('status', help='Show proxy status')

    # stop
    sub.add_parser('stop', help='Stop the running proxy')

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        sys.exit(0)

    if args.command == 'start':
        # Check if already running
        existing = get_proxy_state()
        if existing:
            print(f"CDP Proxy already running (PID {existing['pid']}, "
                  f"port {existing['proxy_port']})")
            sys.exit(0)

        proxy = CDPProxy(
            chrome_port=args.chrome_port,
            proxy_port=args.proxy_port,
        )

        if args.daemon:
            # Daemon mode: run in background with signal handling
            def _handle_signal(signum, frame):
                proxy.stop()
                sys.exit(0)
            signal.signal(signal.SIGTERM, _handle_signal)
            signal.signal(signal.SIGINT, _handle_signal)

        proxy.start(block=True)

    elif args.command == 'status':
        state = get_proxy_state()
        if state:
            age = time.time() - state.get('started_at', 0)
            print(f"✅ CDP Proxy is running")
            print(f"   PID:        {state['pid']}")
            print(f"   Proxy URL:  {state['proxy_url']}")
            print(f"   Chrome:     {state['chrome_host']}:{state['chrome_port']}")
            print(f"   Uptime:     {int(age)}s")
        else:
            print("❌ CDP Proxy is not running")
            sys.exit(1)

    elif args.command == 'stop':
        if stop_proxy():
            print("✅ CDP Proxy stopped.")
        else:
            print("❌ No proxy to stop.")
            sys.exit(1)


if __name__ == '__main__':
    main()
