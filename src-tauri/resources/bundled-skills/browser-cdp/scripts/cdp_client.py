#!/usr/bin/env python3
"""
CDP WebSocket Client — Chrome DevTools Protocol 通信核心层。

提供与 Chromium 内核浏览器的 CDP 通信能力：
- WebSocket 连接管理
- CDP 命令发送与响应接收
- 标签页创建/列出/关闭
- Session 附加/分离

Usage:
    from cdp_client import CDPClient

    client = CDPClient('http://localhost:9222')
    client.connect()
    tabs = client.list_tabs()
    client.attach(tabs[0]['id'])
    result = client.send('Runtime.evaluate', {'expression': 'document.title'})
    client.close()

Run directly for quick test:
    python cdp_client.py --cdp-url http://localhost:9222 --help
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
import _encoding_fix  # noqa: F401

import json
import threading
import time
import argparse
from urllib.request import urlopen, Request
from urllib.error import URLError

try:
    import websockets
    from websockets.sync.client import connect as ws_connect
    HAS_WEBSOCKETS = True
except ImportError:
    # Auto-install websockets if missing
    import subprocess
    try:
        subprocess.check_call(
            [sys.executable, '-m', 'pip', 'install', 'websockets', '-q'],
            stdout=subprocess.DEVNULL,
        )
        import websockets
        from websockets.sync.client import connect as ws_connect
        HAS_WEBSOCKETS = True
    except Exception:
        HAS_WEBSOCKETS = False


# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

CDP_WS_HANDSHAKE_TIMEOUT_S = 5.0
CDP_SEND_TIMEOUT_S = 30.0
CDP_HTTP_TIMEOUT_S = 5.0
CDP_CONNECT_RETRY_COUNT = 3
CDP_CONNECT_RETRY_DELAY_S = 0.5


# ---------------------------------------------------------------------------
# HTTP helpers (for /json/* endpoints)
# ---------------------------------------------------------------------------

def _http_get_json(url: str, timeout: float = CDP_HTTP_TIMEOUT_S):
    """Send GET to a CDP HTTP endpoint and return parsed JSON."""
    req = Request(url, headers={'Accept': 'application/json'})
    with urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode('utf-8'))


def _http_put_json(url: str, timeout: float = CDP_HTTP_TIMEOUT_S):
    """Send PUT to a CDP HTTP endpoint and return parsed JSON."""
    req = Request(url, method='PUT', headers={'Accept': 'application/json'})
    with urlopen(req, timeout=timeout) as resp:
        body = resp.read().decode('utf-8')
        return json.loads(body) if body.strip() else {}


# ---------------------------------------------------------------------------
# CDPClient
# ---------------------------------------------------------------------------

class CDPClient:
    """
    Chrome DevTools Protocol client using synchronous WebSocket.

    Connects to a running Chromium-based browser via CDP and provides
    methods for tab management and protocol command execution.
    """

    def __init__(self, cdp_url: str):
        """
        Args:
            cdp_url: CDP HTTP endpoint, e.g. 'http://localhost:9222'.
                     Used for /json/* REST calls and to discover the
                     browser-level WebSocket URL.
        """
        self.cdp_url = cdp_url.rstrip('/')
        self._ws = None
        self._msg_id = 0
        self._pending = {}          # id -> threading.Event + result holder
        self._lock = threading.Lock()
        self._recv_thread = None
        self._closed = False
        self._events = []           # buffered CDP events
        self._event_lock = threading.Lock()
        self._session_id = None     # current attached session
        self._ws_only = False       # True when HTTP /json/* endpoints are unavailable

    # ------------------------------------------------------------------
    # Connection
    # ------------------------------------------------------------------

    def connect(self):
        """Establish WebSocket connection to the browser-level CDP endpoint.

        Supports three modes:
        1. **HTTP mode** (standard): ``/json/version`` returns the
           ``webSocketDebuggerUrl`` which we use to connect.
        2. **Proxy mode** (Chrome 136+ ``chrome://inspect``): when HTTP
           ``/json/version`` returns data with ``_proxy: true``, we're
           talking to a CDP Proxy.  Connect via the proxy's WS URL.
        3. **WS-only mode** (fallback): HTTP endpoints return 404.
           Automatically start a CDP Proxy to avoid repeated auth dialogs,
           then connect through the proxy.
        """
        if not HAS_WEBSOCKETS:
            raise RuntimeError(
                "Missing 'websockets' package. Install with: pip install websockets"
            )

        ws_url = None

        # Try HTTP /json/version first (standard mode or proxy mode)
        try:
            version_info = self._get_version()
            ws_url = version_info.get('webSocketDebuggerUrl')
            if version_info.get('_proxy'):
                # Already going through a proxy — great
                self._ws_only = False
        except Exception:
            pass  # HTTP failed — probably WS-only mode

        # Fallback: WS-only mode → use CDP Proxy to avoid repeated dialogs
        if not ws_url:
            self._ws_only = True
            ws_url = self._connect_via_proxy()

        last_err = None
        for attempt in range(CDP_CONNECT_RETRY_COUNT):
            try:
                self._ws = ws_connect(
                    ws_url,
                    open_timeout=CDP_WS_HANDSHAKE_TIMEOUT_S,
                    max_size=64 * 1024 * 1024,  # 64 MB max message
                )
                break
            except Exception as e:
                last_err = e
                if attempt < CDP_CONNECT_RETRY_COUNT - 1:
                    time.sleep(CDP_CONNECT_RETRY_DELAY_S * (attempt + 1))

        if self._ws is None:
            raise RuntimeError(
                f"Failed to connect to CDP WebSocket after {CDP_CONNECT_RETRY_COUNT} "
                f"attempts: {last_err}"
            )

        self._closed = False
        self._recv_thread = threading.Thread(
            target=self._recv_loop, daemon=True, name='cdp-recv'
        )
        self._recv_thread.start()

    def _connect_via_proxy(self) -> str:
        """
        Ensure a CDP Proxy is running and return its WS URL.

        In WS-only mode (Chrome 136+ chrome://inspect), each direct WS
        connection triggers an auth dialog.  The proxy maintains a single
        upstream connection so users only see the dialog once.
        """
        from urllib.parse import urlparse
        parsed = urlparse(self.cdp_url)
        chrome_port = parsed.port or 9222

        try:
            from cdp_proxy import ensure_proxy_running, get_proxy_state
            proxy_url = ensure_proxy_running(chrome_port=chrome_port)
            # Update cdp_url to point to the proxy so HTTP endpoints work
            self.cdp_url = proxy_url
            self._ws_only = False  # Proxy provides HTTP endpoints
            proxy_parsed = urlparse(proxy_url)
            proxy_host = proxy_parsed.hostname or '127.0.0.1'
            proxy_port = proxy_parsed.port or 9223
            # Read auth token from proxy state file for secure WS connection
            ws_url = f'ws://{proxy_host}:{proxy_port}/devtools/browser'
            state = get_proxy_state()
            if state and state.get('auth_token'):
                ws_url += f'?token={state["auth_token"]}'
            return ws_url
        except Exception:
            # Proxy not available — fall back to direct WS connection
            # (will trigger auth dialog)
            host = parsed.hostname or '127.0.0.1'
            port = parsed.port or 9222
            return f'ws://{host}:{port}/devtools/browser'

    def close(self):
        """Close the WebSocket connection."""
        self._closed = True
        if self._ws:
            try:
                self._ws.close()
            except Exception:
                pass
            self._ws = None

    @property
    def connected(self) -> bool:
        return self._ws is not None and not self._closed

    # ------------------------------------------------------------------
    # CDP Command
    # ------------------------------------------------------------------

    def send(self, method: str, params: dict = None, timeout: float = CDP_SEND_TIMEOUT_S) -> dict:
        """
        Send a CDP command and wait for its response.

        Args:
            method:  CDP method, e.g. 'Page.navigate'
            params:  Method parameters dict
            timeout: Max wait time in seconds

        Returns:
            The 'result' dict from the CDP response.

        Raises:
            RuntimeError on timeout or CDP error.
        """
        if not self.connected:
            raise RuntimeError("Not connected to CDP")

        with self._lock:
            self._msg_id += 1
            msg_id = self._msg_id

        msg = {'id': msg_id, 'method': method}
        if params:
            msg['params'] = params
        if self._session_id:
            msg['sessionId'] = self._session_id

        event = threading.Event()
        holder = {'result': None, 'error': None}
        self._pending[msg_id] = (event, holder)

        self._ws.send(json.dumps(msg))

        if not event.wait(timeout=timeout):
            self._pending.pop(msg_id, None)
            raise RuntimeError(
                f"CDP command '{method}' timed out after {timeout}s (id={msg_id})"
            )

        self._pending.pop(msg_id, None)

        if holder['error']:
            err = holder['error']
            raise RuntimeError(
                f"CDP error for '{method}': [{err.get('code')}] {err.get('message')}"
            )

        return holder['result'] or {}

    # ------------------------------------------------------------------
    # Tab management (via HTTP /json/* endpoints)
    # ------------------------------------------------------------------

    def list_tabs(self) -> list:
        """List all open browser tabs/targets.

        Uses HTTP ``/json/list`` in standard mode; falls back to
        ``Target.getTargets`` CDP command in WS-only mode.
        """
        if not self._ws_only:
            try:
                targets = _http_get_json(f"{self.cdp_url}/json/list")
                return [
                    {
                        'id': t.get('id', ''),
                        'type': t.get('type', ''),
                        'title': t.get('title', ''),
                        'url': t.get('url', ''),
                        'webSocketDebuggerUrl': t.get('webSocketDebuggerUrl', ''),
                    }
                    for t in targets
                    if t.get('type') == 'page'
                ]
            except Exception:
                pass  # Fall through to CDP command

        # WS-only mode: use CDP command
        if not self.connected:
            raise RuntimeError("Not connected — call connect() first")
        old_session = self._session_id
        self._session_id = None  # Send at browser level
        try:
            result = self.send('Target.getTargets')
        finally:
            self._session_id = old_session
        targets = result.get('targetInfos', [])
        return [
            {
                'id': t.get('targetId', ''),
                'type': t.get('type', ''),
                'title': t.get('title', ''),
                'url': t.get('url', ''),
                'webSocketDebuggerUrl': '',
            }
            for t in targets
            if t.get('type') == 'page'
        ]

    def create_tab(self, url: str = 'about:blank') -> dict:
        """Create a new tab and return its target info.

        Uses HTTP ``/json/new`` in standard mode; falls back to
        ``Target.createTarget`` CDP command in WS-only mode.
        """
        if not self._ws_only:
            try:
                from urllib.parse import quote
                endpoint = f"{self.cdp_url}/json/new?{quote(url, safe='')}"
                # Chrome headless requires PUT; fall back to GET for older versions
                try:
                    target = _http_put_json(endpoint)
                except Exception:
                    target = _http_get_json(endpoint)
                return {
                    'id': target.get('id', ''),
                    'type': target.get('type', ''),
                    'title': target.get('title', ''),
                    'url': target.get('url', ''),
                    'webSocketDebuggerUrl': target.get('webSocketDebuggerUrl', ''),
                }
            except Exception:
                pass  # Fall through to CDP command

        # WS-only mode: use CDP command
        if not self.connected:
            raise RuntimeError("Not connected — call connect() first")
        old_session = self._session_id
        self._session_id = None  # Send at browser level
        try:
            result = self.send('Target.createTarget', {'url': url})
        finally:
            self._session_id = old_session
        target_id = result.get('targetId', '')
        return {
            'id': target_id,
            'type': 'page',
            'title': '',
            'url': url,
            'webSocketDebuggerUrl': '',
        }

    def close_tab(self, target_id: str):
        """Close a tab by target ID.

        Uses HTTP ``/json/close`` in standard mode; falls back to
        ``Target.closeTarget`` CDP command in WS-only mode.
        """
        if not self._ws_only:
            try:
                url = f"{self.cdp_url}/json/close/{target_id}"
                req = Request(url, headers={'Accept': 'application/json'})
                with urlopen(req, timeout=CDP_HTTP_TIMEOUT_S) as resp:
                    resp.read()
                return
            except Exception:
                pass  # Fall through to CDP command

        # WS-only mode: use CDP command
        if not self.connected:
            raise RuntimeError("Not connected — call connect() first")
        old_session = self._session_id
        self._session_id = None
        try:
            self.send('Target.closeTarget', {'targetId': target_id})
        finally:
            self._session_id = old_session

    def activate_tab(self, target_id: str):
        """Bring a tab to foreground.

        Uses HTTP ``/json/activate`` in standard mode; falls back to
        ``Target.activateTarget`` CDP command in WS-only mode.
        """
        if not self._ws_only:
            try:
                _http_get_json(f"{self.cdp_url}/json/activate/{target_id}")
                return
            except Exception:
                pass

        if not self.connected:
            raise RuntimeError("Not connected — call connect() first")
        old_session = self._session_id
        self._session_id = None
        try:
            self.send('Target.activateTarget', {'targetId': target_id})
        finally:
            self._session_id = old_session

    # ------------------------------------------------------------------
    # Session management (attach to a specific target)
    # ------------------------------------------------------------------

    def attach(self, target_id: str):
        """Attach to a target to send page-level CDP commands."""
        result = self.send('Target.attachToTarget', {
            'targetId': target_id,
            'flatten': True,
        })
        self._session_id = result.get('sessionId')
        if not self._session_id:
            raise RuntimeError(f"Failed to attach to target {target_id}: no sessionId returned")

    def detach(self):
        """Detach from the current target session."""
        if self._session_id:
            try:
                old_session = self._session_id
                self._session_id = None
                self.send('Target.detachFromTarget', {'sessionId': old_session})
            except Exception:
                pass
            self._session_id = None

    # ------------------------------------------------------------------
    # Event handling
    # ------------------------------------------------------------------

    def get_events(self, method: str = None, clear: bool = True) -> list:
        """
        Get buffered CDP events, optionally filtered by method.

        Args:
            method: Filter events by this method name (e.g. 'Page.loadEventFired')
            clear:  Whether to clear matched events from the buffer
        """
        with self._event_lock:
            if method:
                matched = [e for e in self._events if e.get('method') == method]
                if clear:
                    self._events = [e for e in self._events if e.get('method') != method]
            else:
                matched = list(self._events)
                if clear:
                    self._events.clear()
        return matched

    def wait_for_event(self, method: str, timeout: float = 30.0) -> dict:
        """Wait for a specific CDP event."""
        deadline = time.time() + timeout
        while time.time() < deadline:
            events = self.get_events(method, clear=True)
            if events:
                return events[0]
            time.sleep(0.1)
        raise RuntimeError(f"Timeout waiting for CDP event '{method}' after {timeout}s")

    # ------------------------------------------------------------------
    # Info
    # ------------------------------------------------------------------

    def get_version(self) -> dict:
        """Get browser version info."""
        return self._get_version()

    def _get_version(self) -> dict:
        return _http_get_json(f"{self.cdp_url}/json/version")

    # ------------------------------------------------------------------
    # Internal: receive loop
    # ------------------------------------------------------------------

    def _recv_loop(self):
        """Background thread that reads WebSocket messages and dispatches them."""
        while not self._closed and self._ws:
            try:
                raw = self._ws.recv(timeout=1.0)
            except TimeoutError:
                continue
            except Exception as e:
                if not self._closed:
                    # ConnectionClosed* (browser killed / normal shutdown)
                    # is expected — don't spam stderr with a traceback.
                    cls_name = type(e).__name__
                    if 'ConnectionClosed' in cls_name:
                        pass  # silent — normal teardown
                    else:
                        import traceback
                        print(f"[cdp-client] _recv_loop error: {cls_name}: {e}",
                              file=__import__('sys').stderr, flush=True)
                        traceback.print_exc(file=__import__('sys').stderr)
                    self._closed = True
                break

            try:
                msg = json.loads(raw)
            except json.JSONDecodeError:
                continue

            # Response to a command
            if 'id' in msg:
                msg_id = msg['id']
                pending = self._pending.get(msg_id)
                if pending:
                    event, holder = pending
                    if 'error' in msg:
                        holder['error'] = msg['error']
                    else:
                        holder['result'] = msg.get('result')
                    event.set()

            # CDP event (no id, has method)
            elif 'method' in msg:
                with self._event_lock:
                    self._events.append(msg)
                    # Keep buffer bounded
                    if len(self._events) > 1000:
                        self._events = self._events[-500:]


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description='CDP Client — interact with a Chromium browser via CDP'
    )
    parser.add_argument(
        '--cdp-url', default='http://localhost:9222',
        help='CDP HTTP endpoint (default: http://localhost:9222)'
    )
    sub = parser.add_subparsers(dest='command')

    sub.add_parser('version', help='Show browser version info')
    sub.add_parser('tabs', help='List open tabs')

    open_p = sub.add_parser('open', help='Open a new tab')
    open_p.add_argument('url', help='URL to open')

    close_p = sub.add_parser('close', help='Close a tab')
    close_p.add_argument('target_id', help='Target ID to close')

    eval_p = sub.add_parser('eval', help='Evaluate JavaScript in the first tab')
    eval_p.add_argument('expression', help='JS expression to evaluate')

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        sys.exit(0)

    client = CDPClient(args.cdp_url)

    if args.command == 'version':
        print(json.dumps(client.get_version(), indent=2, ensure_ascii=False))

    elif args.command == 'tabs':
        tabs = client.list_tabs()
        for i, t in enumerate(tabs):
            print(f"  [{i}] {t['title']}")
            print(f"      URL: {t['url']}")
            print(f"      ID:  {t['id']}")
        if not tabs:
            print("  (no tabs found)")

    elif args.command == 'open':
        tab = client.create_tab(args.url)
        print(f"Opened tab: {tab['id']} → {args.url}")

    elif args.command == 'close':
        client.close_tab(args.target_id)
        print(f"Closed tab: {args.target_id}")

    elif args.command == 'eval':
        client.connect()
        tabs = client.list_tabs()
        if not tabs:
            print("No tabs available")
            sys.exit(1)
        client.attach(tabs[0]['id'])
        result = client.send('Runtime.evaluate', {
            'expression': args.expression,
            'returnByValue': True,
        })
        val = result.get('result', {})
        if val.get('type') == 'undefined':
            print('undefined')
        elif 'value' in val:
            print(json.dumps(val['value'], indent=2, ensure_ascii=False))
        else:
            print(json.dumps(val, indent=2, ensure_ascii=False))
        client.close()


if __name__ == '__main__':
    main()
