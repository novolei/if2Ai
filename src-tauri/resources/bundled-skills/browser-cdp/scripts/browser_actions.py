#!/usr/bin/env python3
"""
Browser Actions — 通过 CDP 实现页面交互操作。

提供导航、点击、输入、截图、JS执行、标签页管理等能力。
支持通过 Accessibility 快照的 ref 编号定位元素。

Usage:
    from cdp_client import CDPClient
    from page_snapshot import PageSnapshot
    from browser_actions import BrowserActions

    client = CDPClient('http://localhost:9222')
    client.connect()
    client.attach(target_id)

    snapshot = PageSnapshot(client)
    actions = BrowserActions(client, snapshot)

    actions.navigate('https://example.com')
    actions.wait_for_load()
    tree = snapshot.accessibility_tree()
    actions.click_by_ref('e1')
    actions.type_text('hello')
    actions.screenshot('/tmp/page.png')

Run directly:
    python browser_actions.py --cdp-url http://localhost:9222 --help
"""

import base64
import json
import os
import sys
import time
import argparse

from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
import _encoding_fix  # noqa: F401
from cdp_client import CDPClient
from page_snapshot import PageSnapshot


# ---------------------------------------------------------------------------
# Key name mapping (common key names → CDP key codes)
# ---------------------------------------------------------------------------

KEY_DEFINITIONS = {
    'enter': {'key': 'Enter', 'code': 'Enter', 'keyCode': 13, 'text': '\r'},
    'return': {'key': 'Enter', 'code': 'Enter', 'keyCode': 13, 'text': '\r'},
    'tab': {'key': 'Tab', 'code': 'Tab', 'keyCode': 9},
    'escape': {'key': 'Escape', 'code': 'Escape', 'keyCode': 27},
    'esc': {'key': 'Escape', 'code': 'Escape', 'keyCode': 27},
    'backspace': {'key': 'Backspace', 'code': 'Backspace', 'keyCode': 8},
    'delete': {'key': 'Delete', 'code': 'Delete', 'keyCode': 46},
    'space': {'key': ' ', 'code': 'Space', 'keyCode': 32, 'text': ' '},
    'arrowup': {'key': 'ArrowUp', 'code': 'ArrowUp', 'keyCode': 38},
    'arrowdown': {'key': 'ArrowDown', 'code': 'ArrowDown', 'keyCode': 40},
    'arrowleft': {'key': 'ArrowLeft', 'code': 'ArrowLeft', 'keyCode': 37},
    'arrowright': {'key': 'ArrowRight', 'code': 'ArrowRight', 'keyCode': 39},
    'home': {'key': 'Home', 'code': 'Home', 'keyCode': 36},
    'end': {'key': 'End', 'code': 'End', 'keyCode': 35},
    'pageup': {'key': 'PageUp', 'code': 'PageUp', 'keyCode': 33},
    'pagedown': {'key': 'PageDown', 'code': 'PageDown', 'keyCode': 34},
    'f1': {'key': 'F1', 'code': 'F1', 'keyCode': 112},
    'f2': {'key': 'F2', 'code': 'F2', 'keyCode': 113},
    'f3': {'key': 'F3', 'code': 'F3', 'keyCode': 114},
    'f4': {'key': 'F4', 'code': 'F4', 'keyCode': 115},
    'f5': {'key': 'F5', 'code': 'F5', 'keyCode': 116},
    'f12': {'key': 'F12', 'code': 'F12', 'keyCode': 123},
}


def _resolve_key(key_name: str) -> dict:
    """Resolve a key name to its CDP key definition."""
    lower = key_name.lower().replace(' ', '')
    if lower in KEY_DEFINITIONS:
        return KEY_DEFINITIONS[lower]
    # Single character
    if len(key_name) == 1:
        code = ord(key_name)
        return {
            'key': key_name,
            'code': f'Key{key_name.upper()}' if key_name.isalpha() else '',
            'keyCode': code,
            'text': key_name,
        }
    # Fallback
    return {'key': key_name, 'code': '', 'keyCode': 0}


# ---------------------------------------------------------------------------
# BrowserActions
# ---------------------------------------------------------------------------

class BrowserActions:
    """
    Page interaction actions via CDP protocol.
    """

    def __init__(self, client: CDPClient, snapshot: PageSnapshot = None):
        """
        Args:
            client: Connected and attached CDPClient
            snapshot: PageSnapshot instance (for ref-based interactions)
        """
        self._client = client
        self._snapshot = snapshot

    # ------------------------------------------------------------------
    # Navigation
    # ------------------------------------------------------------------

    def navigate(self, url: str, wait_until: str = 'load', timeout: float = 30.0):
        """
        Navigate to a URL.

        Args:
            url: Target URL
            wait_until: Not directly used (CDP navigates immediately)
            timeout: Max wait time for navigation
        """
        # Security: block dangerous URL schemes
        from urllib.parse import urlparse
        parsed = urlparse(url)
        ALLOWED_SCHEMES = {'http', 'https', 'about', 'chrome', 'edge'}
        if parsed.scheme and parsed.scheme.lower() not in ALLOWED_SCHEMES:
            raise ValueError(
                f"Blocked navigation to '{parsed.scheme}://' URL. "
                f"Only {', '.join(sorted(ALLOWED_SCHEMES))} schemes are allowed."
            )
        # Security: block SSRF to cloud metadata and private networks
        if parsed.hostname:
            import ipaddress
            try:
                ip = ipaddress.ip_address(parsed.hostname)
                if ip.is_private and parsed.hostname != '127.0.0.1':
                    raise ValueError(f"Blocked navigation to private IP: {parsed.hostname}")
            except ValueError as ve:
                if 'Blocked navigation' in str(ve):
                    raise  # re-raise our security block
                pass  # hostname is not an IP, allow
            BLOCKED_HOSTS = {'metadata.google.internal', '169.254.169.254'}
            if parsed.hostname in BLOCKED_HOSTS:
                raise ValueError(f"Blocked navigation to metadata endpoint: {parsed.hostname}")
        self._client.send('Page.enable')
        result = self._client.send('Page.navigate', {'url': url})

        error_text = result.get('errorText')
        if error_text:
            raise RuntimeError(f"Navigation failed: {error_text}")

        # Wait for page load
        self.wait_for_load(timeout=timeout)

    def go_back(self):
        """Navigate back in history."""
        history = self._client.send('Page.getNavigationHistory')
        current = history.get('currentIndex', 0)
        if current > 0:
            entries = history.get('entries', [])
            self._client.send('Page.navigateToHistoryEntry', {
                'entryId': entries[current - 1]['id']
            })

    def go_forward(self):
        """Navigate forward in history."""
        history = self._client.send('Page.getNavigationHistory')
        current = history.get('currentIndex', 0)
        entries = history.get('entries', [])
        if current < len(entries) - 1:
            self._client.send('Page.navigateToHistoryEntry', {
                'entryId': entries[current + 1]['id']
            })

    def reload(self, ignore_cache: bool = False):
        """Reload the current page."""
        self._client.send('Page.reload', {
            'ignoreCache': ignore_cache,
        })
        self.wait_for_load()

    # ------------------------------------------------------------------
    # Waiting
    # ------------------------------------------------------------------

    def wait_for_load(self, timeout: float = 30.0):
        """Wait for the page to finish loading."""
        try:
            self._client.send('Page.enable')
            # Check if already loaded
            result = self._client.send('Runtime.evaluate', {
                'expression': 'document.readyState',
                'returnByValue': True,
            })
            state = result.get('result', {}).get('value', '')
            if state == 'complete':
                return

            # Wait for loadEventFired
            self._client.wait_for_event('Page.loadEventFired', timeout=timeout)
        except RuntimeError:
            # Timeout — page may still be usable
            pass

    def wait_for_selector(self, selector: str, timeout: float = 10.0) -> bool:
        """
        Wait for a CSS selector to appear in the DOM.

        Returns:
            True if element found, False if timeout.
        """
        deadline = time.time() + timeout
        while time.time() < deadline:
            result = self._client.send('Runtime.evaluate', {
                'expression': f'!!document.querySelector("{selector}")',
                'returnByValue': True,
            })
            if result.get('result', {}).get('value') is True:
                return True
            time.sleep(0.3)
        return False

    def wait_for_navigation(self, timeout: float = 30.0):
        """Wait for a navigation event (page URL change)."""
        self._client.send('Page.enable')
        self._client.wait_for_event('Page.frameNavigated', timeout=timeout)
        self.wait_for_load(timeout=timeout)

    def wait(self, seconds: float):
        """Simple sleep."""
        time.sleep(seconds)

    # ------------------------------------------------------------------
    # Mouse interactions
    # ------------------------------------------------------------------

    def click(self, x: float, y: float, button: str = 'left', click_count: int = 1):
        """
        Click at coordinates.

        Args:
            x, y: Page coordinates
            button: 'left', 'right', or 'middle'
            click_count: Number of clicks (2 for double-click)
        """
        # Move to position
        self._client.send('Input.dispatchMouseEvent', {
            'type': 'mouseMoved',
            'x': x,
            'y': y,
        })
        # Press
        self._client.send('Input.dispatchMouseEvent', {
            'type': 'mousePressed',
            'x': x,
            'y': y,
            'button': button,
            'clickCount': click_count,
        })
        # Release
        self._client.send('Input.dispatchMouseEvent', {
            'type': 'mouseReleased',
            'x': x,
            'y': y,
            'button': button,
            'clickCount': click_count,
        })

    def click_selector(self, selector: str, timeout: float = 5.0):
        """Click an element found by CSS selector."""
        # Get element position via JS
        result = self._client.send('Runtime.evaluate', {
            'expression': f'''
                (function() {{
                    const el = document.querySelector("{selector}");
                    if (!el) return null;
                    const rect = el.getBoundingClientRect();
                    return {{
                        x: rect.x + rect.width / 2,
                        y: rect.y + rect.height / 2,
                    }};
                }})()
            ''',
            'returnByValue': True,
        })

        pos = result.get('result', {}).get('value')
        if not pos:
            raise RuntimeError(f"Element not found: {selector}")

        self.click(pos['x'], pos['y'])

    def click_by_ref(self, ref: str):
        """
        Click an element by its accessibility snapshot ref (e.g. 'e1').

        Uses the snapshot's ref tracking to resolve element identity.
        Multi-strategy cascade:
          1. backendDOMNodeId → coordinates (with elementFromPoint hit-test)
          2. backendDOMNodeId → DOM-level focus+click (bypasses overlays)
          3. JS heuristic matching by role/name → coordinates
        """
        if not self._snapshot:
            raise RuntimeError("No PageSnapshot instance available for ref-based interaction")

        refs = self._snapshot.refs
        if ref not in refs:
            raise RuntimeError(
                f"Ref '{ref}' not found. Available refs: {list(refs.keys())[:10]}... "
                f"Re-run accessibility_tree() to refresh."
            )

        node_info = refs[ref]
        role = node_info['role']
        name = node_info['name']
        backend_node_id = node_info.get('backendDOMNodeId')

        # ----------------------------------------------------------
        # Strategy 1 (primary): Resolve via backendDOMNodeId → coord click
        # with elementFromPoint hit-test to detect occlusion
        # ----------------------------------------------------------
        object_id = None
        if backend_node_id is not None:
            object_id, pos = self._resolve_position_and_object(backend_node_id)
            if pos:
                # Verify the coordinate actually hits our element
                if object_id and self._verify_hit(object_id, pos['x'], pos['y']):
                    self.click(pos['x'], pos['y'])
                    return
                # Hit-test failed → element likely occluded, try DOM click first
                if object_id and self._dom_click(object_id):
                    return
                # DOM click also failed? Fall back to coord click anyway
                # (elementFromPoint is not always reliable across iframes)
                self.click(pos['x'], pos['y'])
                return

        # ----------------------------------------------------------
        # Strategy 2: DOM-level click via objectId (no coordinates needed,
        # bypasses overlays, works in Shadow DOM)
        # ----------------------------------------------------------
        if object_id:
            if self._dom_click(object_id):
                return

        # If we have backend_node_id but no object_id yet, try to get one
        if backend_node_id is not None and object_id is None:
            object_id = self._resolve_object_id(backend_node_id)
            if object_id and self._dom_click(object_id):
                return

        # ----------------------------------------------------------
        # Strategy 3 (fallback): JS heuristic matching
        # ----------------------------------------------------------
        pos = self._resolve_position_by_js(role, name)
        if pos:
            self.click(pos['x'], pos['y'])
            return

        raise RuntimeError(
            f"Could not locate element for ref '{ref}' (role={role}, name={name}). "
            f"The page may have changed. Try refreshing the snapshot."
        )

    def _resolve_object_id(self, backend_node_id: int) -> str:
        """
        Resolve backendDOMNodeId → JS objectId via CDP DOM.resolveNode.

        Returns:
            objectId string, or None if resolution fails.
        """
        try:
            self._client.send('DOM.enable')
            resolve_result = self._client.send('DOM.resolveNode', {
                'backendNodeId': backend_node_id,
            })
            return resolve_result.get('object', {}).get('objectId')
        except Exception:
            return None

    def _resolve_position_and_object(self, backend_node_id: int) -> tuple:
        """
        Resolve element center coordinates AND objectId via CDP DOM.resolveNode.

        This is the most reliable method — it directly maps the AX tree
        node to its DOM element without any guessing.

        Returns:
            (objectId, {'x': float, 'y': float}) or (objectId, None) or (None, None)
        """
        try:
            self._client.send('DOM.enable')

            # Resolve backendDOMNodeId → JS object reference
            resolve_result = self._client.send('DOM.resolveNode', {
                'backendNodeId': backend_node_id,
            })
            object_id = resolve_result.get('object', {}).get('objectId')
            if not object_id:
                return None, None

            # Call getBoundingClientRect on the resolved element
            fn_result = self._client.send('Runtime.callFunctionOn', {
                'objectId': object_id,
                'functionDeclaration': '''function() {
                    const rect = this.getBoundingClientRect();
                    if (rect.width === 0 && rect.height === 0) return null;
                    return {
                        x: rect.x + rect.width / 2,
                        y: rect.y + rect.height / 2,
                        width: rect.width,
                        height: rect.height,
                    };
                }''',
                'returnByValue': True,
            })

            pos = fn_result.get('result', {}).get('value')
            if pos and pos.get('width', 0) > 0 and pos.get('height', 0) > 0:
                return object_id, {'x': pos['x'], 'y': pos['y']}

            # Element has zero size — try to scroll it into view first
            self._client.send('Runtime.callFunctionOn', {
                'objectId': object_id,
                'functionDeclaration': '''function() {
                    this.scrollIntoViewIfNeeded
                        ? this.scrollIntoViewIfNeeded(true)
                        : this.scrollIntoView({block: "center", inline: "center"});
                }''',
            })
            time.sleep(0.3)

            # Retry after scroll
            fn_result = self._client.send('Runtime.callFunctionOn', {
                'objectId': object_id,
                'functionDeclaration': '''function() {
                    const rect = this.getBoundingClientRect();
                    if (rect.width === 0 && rect.height === 0) return null;
                    return {
                        x: rect.x + rect.width / 2,
                        y: rect.y + rect.height / 2,
                    };
                }''',
                'returnByValue': True,
            })
            pos = fn_result.get('result', {}).get('value')
            return object_id, pos

        except Exception:
            # DOM.resolveNode can fail if the node has been detached
            return None, None

    def _verify_hit(self, object_id: str, x: float, y: float) -> bool:
        """
        Verify that elementFromPoint(x, y) hits the target element (or a descendant).

        This detects occlusion by overlays, modals, or floating elements.

        Returns:
            True if the coordinate hits the target, False otherwise.
        """
        try:
            result = self._client.send('Runtime.callFunctionOn', {
                'objectId': object_id,
                'functionDeclaration': '''function(px, py) {
                    const hit = document.elementFromPoint(px, py);
                    if (!hit) return false;
                    // hit is us, or a descendant of us, or we are a descendant of hit
                    return this === hit || this.contains(hit) || hit.contains(this);
                }''',
                'arguments': [
                    {'value': x},
                    {'value': y},
                ],
                'returnByValue': True,
            })
            return result.get('result', {}).get('value', False) is True
        except Exception:
            # If verification fails, don't block — assume hit
            return True

    def _dom_click(self, object_id: str) -> bool:
        """
        Perform a DOM-level focus + click on the element via its objectId.

        This bypasses coordinate-based clicking entirely — it works even when:
        - The element is obscured by overlays
        - The element is inside Shadow DOM
        - The element has zero visual size but is still interactive

        Returns:
            True if the click was dispatched, False on failure.
        """
        try:
            result = self._client.send('Runtime.callFunctionOn', {
                'objectId': object_id,
                'functionDeclaration': '''function() {
                    // Scroll into view if needed
                    if (this.scrollIntoViewIfNeeded) {
                        this.scrollIntoViewIfNeeded(true);
                    } else if (this.scrollIntoView) {
                        this.scrollIntoView({block: "center", inline: "center"});
                    }
                    // Focus the element
                    if (this.focus) this.focus();
                    // Dispatch a full click event sequence
                    this.dispatchEvent(new MouseEvent('mousedown', {bubbles: true, cancelable: true, view: window}));
                    this.dispatchEvent(new MouseEvent('mouseup', {bubbles: true, cancelable: true, view: window}));
                    this.dispatchEvent(new MouseEvent('click', {bubbles: true, cancelable: true, view: window}));
                    return true;
                }''',
                'returnByValue': True,
            })
            return result.get('result', {}).get('value', False) is True
        except Exception:
            return False

    def _resolve_position_by_js(self, role: str, name: str) -> dict:
        """
        Fallback: find element position via JS heuristic matching.

        Tries aria-label, role attribute, and semantic tag mapping.

        Returns:
            {'x': float, 'y': float} or None if not found.
        """
        # Escape quotes in name for safe JS string embedding
        safe_name = name.replace('\\', '\\\\').replace('"', '\\"').replace("'", "\\'")

        js_code = f'''
        (function() {{
            // Strategy A: aria-label match
            try {{
                const byLabel = document.querySelectorAll('[aria-label="{safe_name}"]');
                for (const el of byLabel) {{
                    if (el.getAttribute('role') === '{role}' || !'{role}') {{
                        const rect = el.getBoundingClientRect();
                        if (rect.width > 0 && rect.height > 0) {{
                            return {{ x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 }};
                        }}
                    }}
                }}
            }} catch(e) {{}}

            // Strategy B: role attribute match + text content
            try {{
                const byRole = document.querySelectorAll('[role="{role}"]');
                for (const el of byRole) {{
                    if (el.textContent.trim().includes("{safe_name}") || el.getAttribute('aria-label') === "{safe_name}") {{
                        const rect = el.getBoundingClientRect();
                        if (rect.width > 0 && rect.height > 0) {{
                            return {{ x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 }};
                        }}
                    }}
                }}
            }} catch(e) {{}}

            // Strategy C: Semantic tag mapping
            const tagMap = {{
                'button': 'button,input[type="button"],input[type="submit"]',
                'link': 'a',
                'textbox': 'input[type="text"],input[type="search"],input[type="email"],input[type="password"],input:not([type]),textarea',
                'checkbox': 'input[type="checkbox"]',
                'radio': 'input[type="radio"]',
                'combobox': 'select',
                'searchbox': 'input[type="search"]',
                'img': 'img',
            }};
            const tagSelector = tagMap['{role}'];
            if (tagSelector) {{
                try {{
                    const candidates = document.querySelectorAll(tagSelector);
                    for (const el of candidates) {{
                        const elName = el.getAttribute('aria-label') ||
                                       el.getAttribute('title') ||
                                       el.getAttribute('placeholder') ||
                                       el.textContent.trim();
                        if (elName && elName.includes("{safe_name}")) {{
                            const rect = el.getBoundingClientRect();
                            if (rect.width > 0 && rect.height > 0) {{
                                return {{ x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 }};
                            }}
                        }}
                    }}
                }} catch(e) {{}}
            }}

            return null;
        }})()
        '''

        result = self._client.send('Runtime.evaluate', {
            'expression': js_code,
            'returnByValue': True,
        })

        return result.get('result', {}).get('value')

    def hover(self, x: float, y: float):
        """Move mouse to coordinates (hover)."""
        self._client.send('Input.dispatchMouseEvent', {
            'type': 'mouseMoved',
            'x': x,
            'y': y,
        })

    def drag(self, from_x: float, from_y: float, to_x: float, to_y: float, steps: int = 10):
        """Drag from one position to another."""
        # Press at start
        self._client.send('Input.dispatchMouseEvent', {
            'type': 'mousePressed',
            'x': from_x,
            'y': from_y,
            'button': 'left',
        })

        # Move in steps
        for i in range(1, steps + 1):
            t = i / steps
            x = from_x + (to_x - from_x) * t
            y = from_y + (to_y - from_y) * t
            self._client.send('Input.dispatchMouseEvent', {
                'type': 'mouseMoved',
                'x': x,
                'y': y,
            })
            time.sleep(0.02)

        # Release at end
        self._client.send('Input.dispatchMouseEvent', {
            'type': 'mouseReleased',
            'x': to_x,
            'y': to_y,
            'button': 'left',
        })

    def scroll(self, x: float, y: float, delta_x: float = 0, delta_y: float = -200):
        """
        Scroll at a position.

        Args:
            x, y: Position to scroll at
            delta_x: Horizontal scroll amount (negative = right)
            delta_y: Vertical scroll amount (negative = down)
        """
        self._client.send('Input.dispatchMouseEvent', {
            'type': 'mouseWheel',
            'x': x,
            'y': y,
            'deltaX': delta_x,
            'deltaY': delta_y,
        })

    # ------------------------------------------------------------------
    # Keyboard interactions
    # ------------------------------------------------------------------

    def type_text(self, text: str, delay_ms: int = 50):
        """
        Type text character by character.

        Uses ``Input.insertText`` for each character, which is the most
        reliable CDP method for text input — it works correctly for
        all scripts (CJK, emoji, etc.) and avoids the double-input bug
        that occurs when ``keyDown`` carries a ``text`` field alongside
        a separate ``char`` event.

        Args:
            text: Text to type
            delay_ms: Delay between keystrokes in milliseconds
        """
        for char in text:
            # Input.insertText is specifically designed for text input.
            # It inserts text as if it were typed, triggering the correct
            # input/beforeinput events, without duplicating characters.
            self._client.send('Input.insertText', {
                'text': char,
            })
            if delay_ms:
                time.sleep(delay_ms / 1000.0)

    def press_key(self, key: str):
        """
        Press a single key (e.g. 'Enter', 'Tab', 'Escape', 'a').

        Args:
            key: Key name or single character
        """
        key_def = _resolve_key(key)
        # keyDown
        params = {
            'type': 'keyDown',
            'key': key_def.get('key', key),
            'code': key_def.get('code', ''),
            'windowsVirtualKeyCode': key_def.get('keyCode', 0),
        }
        if 'text' in key_def:
            params['text'] = key_def['text']
        self._client.send('Input.dispatchKeyEvent', params)

        # keyUp
        self._client.send('Input.dispatchKeyEvent', {
            'type': 'keyUp',
            'key': key_def.get('key', key),
            'code': key_def.get('code', ''),
            'windowsVirtualKeyCode': key_def.get('keyCode', 0),
        })

    def fill(self, selector: str, value: str):
        """
        Clear an input field and fill it with new value.

        Args:
            selector: CSS selector of the input element
            value: Text value to fill
        """
        # Focus the element
        self._client.send('Runtime.evaluate', {
            'expression': f'''
                (function() {{
                    const el = document.querySelector("{selector}");
                    if (el) {{
                        el.focus();
                        el.value = "";
                        el.dispatchEvent(new Event("input", {{ bubbles: true }}));
                    }}
                }})()
            ''',
        })
        time.sleep(0.1)
        # Type the value
        self.type_text(value, delay_ms=20)

    def select_option(self, selector: str, value: str):
        """Select an option in a <select> element by value."""
        self._client.send('Runtime.evaluate', {
            'expression': f'''
                (function() {{
                    const el = document.querySelector("{selector}");
                    if (el) {{
                        el.value = "{value}";
                        el.dispatchEvent(new Event("change", {{ bubbles: true }}));
                    }}
                }})()
            ''',
        })

    # ------------------------------------------------------------------
    # Screenshot & Visual
    # ------------------------------------------------------------------

    def screenshot(self, path: str = '/tmp/screenshot.png', full_page: bool = False, quality: int = None) -> str:
        """
        Capture a screenshot.

        Args:
            path: Output file path (supports .png and .jpg)
            full_page: Capture the full scrollable page
            quality: JPEG quality (1-100), only for .jpg

        Returns:
            The output file path.
        """
        params = {}

        if full_page:
            # Get full page dimensions
            metrics = self._client.send('Page.getLayoutMetrics')
            content_size = metrics.get('contentSize', {})
            width = content_size.get('width', 1920)
            height = content_size.get('height', 1080)
            params['clip'] = {
                'x': 0, 'y': 0,
                'width': width, 'height': height,
                'scale': 1,
            }

        # Determine format
        if path.lower().endswith(('.jpg', '.jpeg')):
            params['format'] = 'jpeg'
            if quality:
                params['quality'] = quality
        else:
            params['format'] = 'png'

        result = self._client.send('Page.captureScreenshot', params)
        data = result.get('data', '')

        # Decode and save
        img_bytes = base64.b64decode(data)
        os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
        with open(path, 'wb') as f:
            f.write(img_bytes)

        return path

    def pdf(self, path: str = '/tmp/page.pdf', **kwargs) -> str:
        """
        Export page as PDF (headless mode only).

        Args:
            path: Output PDF path
            **kwargs: Additional Page.printToPDF params

        Returns:
            The output file path.
        """
        params = {
            'printBackground': True,
            'preferCSSPageSize': True,
        }
        params.update(kwargs)

        result = self._client.send('Page.printToPDF', params)
        data = result.get('data', '')

        pdf_bytes = base64.b64decode(data)
        os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
        with open(path, 'wb') as f:
            f.write(pdf_bytes)

        return path

    # ------------------------------------------------------------------
    # JavaScript evaluation
    # ------------------------------------------------------------------

    def evaluate(self, expression: str, return_by_value: bool = True):
        """
        Execute JavaScript in the page context.

        Args:
            expression: JS expression or statement
            return_by_value: If True, returns the actual value; otherwise returns remote object

        Returns:
            The evaluation result value.
        """
        result = self._client.send('Runtime.evaluate', {
            'expression': expression,
            'returnByValue': return_by_value,
            'awaitPromise': True,
        })

        remote_obj = result.get('result', {})
        exception = result.get('exceptionDetails')

        if exception:
            text = exception.get('text', '')
            exc_obj = exception.get('exception', {})
            desc = exc_obj.get('description', '')
            raise RuntimeError(f"JS evaluation error: {text} — {desc}")

        if return_by_value:
            return remote_obj.get('value')
        return remote_obj

    # ------------------------------------------------------------------
    # Tab management
    # ------------------------------------------------------------------

    def list_tabs(self) -> list:
        """List all open tabs."""
        return self._client.list_tabs()

    def new_tab(self, url: str = 'about:blank') -> dict:
        """Open a new tab."""
        return self._client.create_tab(url)

    def switch_tab(self, target_id: str):
        """Switch to a different tab."""
        self._client.detach()
        self._client.activate_tab(target_id)
        self._client.attach(target_id)

    def close_tab(self, target_id: str = None):
        """Close a tab (current tab if no ID given)."""
        if target_id:
            self._client.close_tab(target_id)
        else:
            # Close current via JS
            self._client.send('Runtime.evaluate', {
                'expression': 'window.close()',
            })

    # ------------------------------------------------------------------
    # Console & Errors
    # ------------------------------------------------------------------

    def get_console_messages(self, clear: bool = True) -> list:
        """Get buffered console messages."""
        events = self._client.get_events('Runtime.consoleAPICalled', clear=clear)
        messages = []
        for event in events:
            params = event.get('params', {})
            msg_type = params.get('type', 'log')
            args = params.get('args', [])
            text_parts = []
            for arg in args:
                val = arg.get('value', arg.get('description', str(arg.get('type', ''))))
                text_parts.append(str(val))
            messages.append({
                'type': msg_type,
                'text': ' '.join(text_parts),
            })
        return messages

    def enable_console_capture(self):
        """Enable console message capture."""
        self._client.send('Runtime.enable')

    # ------------------------------------------------------------------
    # Page info
    # ------------------------------------------------------------------

    def get_title(self) -> str:
        """Get current page title."""
        result = self._client.send('Runtime.evaluate', {
            'expression': 'document.title',
            'returnByValue': True,
        })
        return str(result.get('result', {}).get('value', ''))

    def get_url(self) -> str:
        """Get current page URL."""
        result = self._client.send('Runtime.evaluate', {
            'expression': 'window.location.href',
            'returnByValue': True,
        })
        return str(result.get('result', {}).get('value', ''))

    def get_cookies(self) -> list:
        """Get all cookies for the current page."""
        result = self._client.send('Network.getCookies')
        return result.get('cookies', [])

    def set_viewport(self, width: int, height: int, device_scale_factor: float = 1.0):
        """Set the viewport size."""
        self._client.send('Emulation.setDeviceMetricsOverride', {
            'width': width,
            'height': height,
            'deviceScaleFactor': device_scale_factor,
            'mobile': False,
        })


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description='Browser Actions — interact with browser pages via CDP'
    )
    parser.add_argument(
        '--cdp-url', default='http://localhost:9222',
        help='CDP HTTP endpoint'
    )
    parser.add_argument(
        '--target', default=None,
        help='Target ID to attach to (default: first tab)'
    )

    sub = parser.add_subparsers(dest='command')

    nav_p = sub.add_parser('navigate', help='Navigate to URL')
    nav_p.add_argument('url', help='URL to navigate to')

    click_p = sub.add_parser('click', help='Click at coordinates')
    click_p.add_argument('x', type=float, help='X coordinate')
    click_p.add_argument('y', type=float, help='Y coordinate')

    click_sel_p = sub.add_parser('click-selector', help='Click by CSS selector')
    click_sel_p.add_argument('selector', help='CSS selector')

    type_p = sub.add_parser('type', help='Type text')
    type_p.add_argument('text', help='Text to type')

    fill_p = sub.add_parser('fill', help='Fill input field')
    fill_p.add_argument('selector', help='CSS selector')
    fill_p.add_argument('value', help='Value to fill')

    key_p = sub.add_parser('press', help='Press a key')
    key_p.add_argument('key', help='Key name (Enter/Tab/Escape/...)')

    shot_p = sub.add_parser('screenshot', help='Take screenshot')
    shot_p.add_argument('--path', default='/tmp/screenshot.png', help='Output path')
    shot_p.add_argument('--full-page', action='store_true', help='Full page capture')

    eval_p = sub.add_parser('eval', help='Evaluate JavaScript')
    eval_p.add_argument('expression', help='JS expression')

    sub.add_parser('tabs', help='List open tabs')
    sub.add_parser('title', help='Get page title')
    sub.add_parser('url', help='Get page URL')

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        sys.exit(0)

    client = CDPClient(args.cdp_url)
    client.connect()

    tabs = client.list_tabs()
    if not tabs:
        print("No tabs available", file=sys.stderr)
        sys.exit(1)

    target_id = args.target or tabs[0]['id']
    client.attach(target_id)

    snapshot = PageSnapshot(client)
    actions = BrowserActions(client, snapshot)

    try:
        if args.command == 'navigate':
            actions.navigate(args.url)
            print(f"Navigated to: {args.url}")
            print(f"Title: {actions.get_title()}")

        elif args.command == 'click':
            actions.click(args.x, args.y)
            print(f"Clicked at ({args.x}, {args.y})")

        elif args.command == 'click-selector':
            actions.click_selector(args.selector)
            print(f"Clicked: {args.selector}")

        elif args.command == 'type':
            actions.type_text(args.text)
            print(f"Typed: {args.text}")

        elif args.command == 'fill':
            actions.fill(args.selector, args.value)
            print(f"Filled {args.selector} with: {args.value}")

        elif args.command == 'press':
            actions.press_key(args.key)
            print(f"Pressed: {args.key}")

        elif args.command == 'screenshot':
            path = actions.screenshot(args.path, full_page=args.full_page)
            print(f"Screenshot saved to: {path}")

        elif args.command == 'eval':
            result = actions.evaluate(args.expression)
            print(json.dumps(result, indent=2, ensure_ascii=False) if result is not None else 'undefined')

        elif args.command == 'tabs':
            tabs_list = actions.list_tabs()
            for i, t in enumerate(tabs_list):
                print(f"  [{i}] {t['title']}")
                print(f"      URL: {t['url']}")

        elif args.command == 'title':
            print(actions.get_title())

        elif args.command == 'url':
            print(actions.get_url())

    finally:
        client.close()


if __name__ == '__main__':
    main()
