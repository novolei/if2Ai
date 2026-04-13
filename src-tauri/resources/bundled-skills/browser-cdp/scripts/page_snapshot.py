#!/usr/bin/env python3
"""
Page Snapshot — 通过 CDP 获取页面结构化快照。

支持两种快照模式：
1. Accessibility Tree — 基于无障碍树，为交互元素分配 ref 编号
2. DOM Snapshot — 基于 DOM 结构，提取标签/属性/文本

Usage:
    from cdp_client import CDPClient
    from page_snapshot import PageSnapshot

    client = CDPClient('http://localhost:9222')
    client.connect()
    client.attach(target_id)

    snapshot = PageSnapshot(client)
    tree = snapshot.accessibility_tree()
    print(tree)

Run directly:
    python page_snapshot.py --cdp-url http://localhost:9222 --help
"""

import json
import sys
import argparse

# Import sibling module
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
import _encoding_fix  # noqa: F401
from cdp_client import CDPClient


# ---------------------------------------------------------------------------
# ARIA Role Classification (ref: snapshot-roles.ts)
# ---------------------------------------------------------------------------

INTERACTIVE_ROLES = frozenset({
    'button', 'checkbox', 'combobox', 'link', 'listbox', 'menuitem',
    'menuitemcheckbox', 'menuitemradio', 'option', 'radio', 'searchbox',
    'slider', 'spinbutton', 'switch', 'tab', 'textbox', 'treeitem',
})

CONTENT_ROLES = frozenset({
    'article', 'cell', 'columnheader', 'gridcell', 'heading', 'img',
    'listitem', 'main', 'navigation', 'region', 'rowheader',
})

STRUCTURAL_ROLES = frozenset({
    'application', 'directory', 'document', 'generic', 'grid', 'group',
    'ignored', 'list', 'menu', 'menubar', 'none', 'presentation', 'row',
    'rowgroup', 'table', 'tablist', 'toolbar', 'tree', 'treegrid',
})


# ---------------------------------------------------------------------------
# Accessibility Tree helpers
# ---------------------------------------------------------------------------

def _get_ax_node_property(node: dict, name: str) -> str:
    """Extract a named property value from an AX node's properties list."""
    for prop in node.get('properties', []):
        if prop.get('name') == name:
            val = prop.get('value', {})
            return str(val.get('value', ''))
    return ''


def _get_ax_node_name(node: dict) -> str:
    """Get the accessible name of an AX node."""
    name_obj = node.get('name', {})
    return str(name_obj.get('value', ''))


def _get_ax_node_role(node: dict) -> str:
    """Get the role of an AX node."""
    role_obj = node.get('role', {})
    return str(role_obj.get('value', ''))


def _get_ax_node_value(node: dict) -> str:
    """Get the value of an AX node."""
    value_obj = node.get('value', {})
    return str(value_obj.get('value', ''))


def _should_assign_ref(role: str, name: str) -> bool:
    """Determine if this node should get an eN ref."""
    if role in INTERACTIVE_ROLES:
        return True
    if role in CONTENT_ROLES and name:
        return True
    return False


def _should_skip_node(role: str, name: str, compact: bool) -> bool:
    """Determine if this node should be skipped in output."""
    if role in ('none', 'presentation', 'ignored', 'generic'):
        if not name and compact:
            return True
    return False


class RefTracker:
    """Track ref assignments and role+name deduplication."""

    def __init__(self):
        self._counter = 0
        self._refs = {}        # ref -> node info
        self._seen = {}        # (role, name) -> count

    def assign(self, role: str, name: str, backend_node_id: int = None) -> str:
        """Assign a new ref and return it (e.g. 'e1').

        Args:
            role: ARIA role of the element
            name: Accessible name of the element
            backend_node_id: CDP backendDOMNodeId for precise DOM resolution
        """
        self._counter += 1
        ref = f'e{self._counter}'

        # Track for deduplication
        key = (role, name)
        self._seen[key] = self._seen.get(key, 0) + 1

        self._refs[ref] = {
            'role': role,
            'name': name,
            'backendDOMNodeId': backend_node_id,
        }
        return ref

    @property
    def refs(self) -> dict:
        return dict(self._refs)


# ---------------------------------------------------------------------------
# PageSnapshot
# ---------------------------------------------------------------------------

class PageSnapshot:
    """
    Capture structured page snapshots via CDP.
    """

    def __init__(self, client: CDPClient):
        self._client = client
        self._ref_tracker = RefTracker()

    @property
    def refs(self) -> dict:
        """Get the current ref → node info mapping."""
        return self._ref_tracker.refs

    def accessibility_tree(
        self,
        max_depth: int = 0,
        interactive_only: bool = False,
        compact: bool = True,
        max_chars: int = 80000,
    ) -> str:
        """
        Get a formatted Accessibility Tree snapshot.

        Args:
            max_depth: Maximum tree depth (0 = unlimited)
            interactive_only: Only include interactive elements
            compact: Skip empty structural nodes
            max_chars: Maximum output character count

        Returns:
            Formatted text representation of the accessibility tree.
        """
        # Enable Accessibility domain and get full tree
        self._client.send('Accessibility.enable')
        result = self._client.send('Accessibility.getFullAXTree')
        nodes = result.get('nodes', [])

        if not nodes:
            return "(empty accessibility tree)"

        # Build parent→children index
        by_id = {}
        children_map = {}  # nodeId -> [child nodes]

        for node in nodes:
            node_id = node.get('nodeId', '')
            by_id[node_id] = node
            children_map[node_id] = []

        for node in nodes:
            parent_id = node.get('parentId')
            if parent_id and parent_id in children_map:
                children_map[parent_id].append(node)

        # Find root nodes (nodes without parents or with missing parents)
        root_nodes = []
        for node in nodes:
            parent_id = node.get('parentId')
            if not parent_id or parent_id not in by_id:
                root_nodes.append(node)

        # Reset ref tracker
        self._ref_tracker = RefTracker()

        # DFS format
        lines = []
        char_count = [0]

        def _format_node(node, depth):
            if max_chars and char_count[0] >= max_chars:
                return
            if max_depth and depth > max_depth:
                return

            role = _get_ax_node_role(node)
            name = _get_ax_node_name(node)
            value = _get_ax_node_value(node)
            node_id = node.get('nodeId', '')

            # Skip logic
            if _should_skip_node(role, name, compact):
                # Still process children (they might be important)
                for child in children_map.get(node_id, []):
                    _format_node(child, depth)
                return

            if interactive_only and role not in INTERACTIVE_ROLES:
                # Still recurse for interactive descendants
                for child in children_map.get(node_id, []):
                    _format_node(child, depth)
                return

            # Build line
            indent = '  ' * depth
            ref_str = ''

            if _should_assign_ref(role, name):
                backend_id = node.get('backendDOMNodeId')
                ref = self._ref_tracker.assign(role, name, backend_node_id=backend_id)
                ref_str = f'[{ref}] '

            name_str = f' "{name}"' if name else ''
            value_str = f': "{value}"' if value and value != name else ''

            line = f'{indent}{ref_str}{role}{name_str}{value_str}'
            lines.append(line)
            char_count[0] += len(line) + 1

            # Recurse children
            for child in children_map.get(node_id, []):
                _format_node(child, depth + 1)

        for root in root_nodes:
            _format_node(root, 0)

        if max_chars and char_count[0] >= max_chars:
            lines.append(f'\n... (truncated at {max_chars} chars)')

        return '\n'.join(lines)

    def dom_snapshot(self, selector: str = 'body', max_depth: int = 6) -> str:
        """
        Get a simplified DOM structure snapshot via JavaScript evaluation.

        Args:
            selector: CSS selector for the root element
            max_depth: Maximum DOM depth to traverse

        Returns:
            Formatted text representation of the DOM.
        """
        js_code = f"""
        (function() {{
            const root = document.querySelector('{selector}');
            if (!root) return '(element not found: {selector})';

            const MAX_DEPTH = {max_depth};
            const lines = [];

            function traverse(el, depth) {{
                if (depth > MAX_DEPTH) return;

                const tag = el.tagName ? el.tagName.toLowerCase() : '';
                if (!tag) return;

                // Skip script/style/svg internals
                if (['script', 'style', 'noscript'].includes(tag)) return;

                const indent = '  '.repeat(depth);
                const id = el.id ? '#' + el.id : '';
                const cls = el.className && typeof el.className === 'string'
                    ? '.' + el.className.trim().split(/\\s+/).join('.')
                    : '';
                const attrs = [];
                if (el.type) attrs.push('type=' + el.type);
                if (el.name) attrs.push('name=' + el.name);
                if (el.href) attrs.push('href=' + el.href.substring(0, 60));
                if (el.src) attrs.push('src=' + el.src.substring(0, 60));
                const attrStr = attrs.length ? ' [' + attrs.join(', ') + ']' : '';

                // Get direct text content (not from children)
                let text = '';
                for (const child of el.childNodes) {{
                    if (child.nodeType === 3) {{
                        const t = child.textContent.trim();
                        if (t) text += t + ' ';
                    }}
                }}
                text = text.trim();
                const textStr = text ? ' "' + text.substring(0, 80) + '"' : '';

                lines.push(indent + tag + id + cls + attrStr + textStr);

                for (const child of el.children) {{
                    traverse(child, depth + 1);
                }}
            }}

            traverse(root, 0);
            return lines.join('\\n');
        }})()
        """

        result = self._client.send('Runtime.evaluate', {
            'expression': js_code,
            'returnByValue': True,
        })

        value = result.get('result', {}).get('value', '')
        if isinstance(value, str):
            return value
        return str(value)

    def get_text(self) -> str:
        """Get the page's visible text content."""
        result = self._client.send('Runtime.evaluate', {
            'expression': 'document.body ? document.body.innerText : ""',
            'returnByValue': True,
        })
        return str(result.get('result', {}).get('value', ''))

    def get_html(self, selector: str = 'html') -> str:
        """Get the outer HTML of an element."""
        result = self._client.send('Runtime.evaluate', {
            'expression': f'''
                (function() {{
                    const el = document.querySelector('{selector}');
                    return el ? el.outerHTML : '(not found)';
                }})()
            ''',
            'returnByValue': True,
        })
        return str(result.get('result', {}).get('value', ''))

    def get_title(self) -> str:
        """Get the page title."""
        result = self._client.send('Runtime.evaluate', {
            'expression': 'document.title',
            'returnByValue': True,
        })
        return str(result.get('result', {}).get('value', ''))

    def get_url(self) -> str:
        """Get the current page URL."""
        result = self._client.send('Runtime.evaluate', {
            'expression': 'window.location.href',
            'returnByValue': True,
        })
        return str(result.get('result', {}).get('value', ''))


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description='Page Snapshot — get structured page snapshots via CDP'
    )
    parser.add_argument(
        '--cdp-url', default='http://localhost:9222',
        help='CDP HTTP endpoint (default: http://localhost:9222)'
    )
    parser.add_argument(
        '--target', default=None,
        help='Target ID to attach to (default: first tab)'
    )

    sub = parser.add_subparsers(dest='command')

    ax_p = sub.add_parser('accessibility', help='Get Accessibility Tree snapshot')
    ax_p.add_argument('--max-depth', type=int, default=0, help='Max tree depth (0=unlimited)')
    ax_p.add_argument('--interactive-only', action='store_true', help='Only interactive elements')
    ax_p.add_argument('--no-compact', action='store_true', help='Show all structural nodes')
    ax_p.add_argument('--max-chars', type=int, default=80000, help='Max output characters')

    dom_p = sub.add_parser('dom', help='Get DOM snapshot')
    dom_p.add_argument('--selector', default='body', help='Root CSS selector')
    dom_p.add_argument('--max-depth', type=int, default=6, help='Max DOM depth')

    sub.add_parser('text', help='Get page text content')
    sub.add_parser('title', help='Get page title')
    sub.add_parser('url', help='Get page URL')

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        sys.exit(0)

    client = CDPClient(args.cdp_url)
    client.connect()

    # Attach to target
    tabs = client.list_tabs()
    if not tabs:
        print("No tabs available", file=sys.stderr)
        sys.exit(1)

    target_id = args.target or tabs[0]['id']
    client.attach(target_id)

    snapshot = PageSnapshot(client)

    if args.command == 'accessibility':
        tree = snapshot.accessibility_tree(
            max_depth=args.max_depth,
            interactive_only=args.interactive_only,
            compact=not args.no_compact,
            max_chars=args.max_chars,
        )
        print(tree)
        # Print ref summary
        refs = snapshot.refs
        if refs:
            print(f"\n--- {len(refs)} refs assigned ---")

    elif args.command == 'dom':
        dom = snapshot.dom_snapshot(
            selector=args.selector,
            max_depth=args.max_depth,
        )
        print(dom)

    elif args.command == 'text':
        print(snapshot.get_text())

    elif args.command == 'title':
        print(snapshot.get_title())

    elif args.command == 'url':
        print(snapshot.get_url())

    client.close()


if __name__ == '__main__':
    main()
