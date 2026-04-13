#!/usr/bin/env python3
"""
Example: Multi-Tab — 在用户浏览器中打开多个标签页并行采集。

典型场景：同时打开多个页面对比信息，或批量采集后汇总。

Usage:
    python examples/multi_tab.py
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / 'scripts'))

from browser_launcher import BrowserLauncher
from cdp_client import CDPClient
from page_snapshot import PageSnapshot
from browser_actions import BrowserActions


def main():
    urls = [
        'https://example.com',
        'https://httpbin.org/html',
        'https://httpbin.org/json',
    ]

    launcher = BrowserLauncher()
    client = None

    try:
        # 复用用户已有的 Chrome
        print("Connecting to user's browser...")
        cdp_url = launcher.launch(browser='chrome', reuse_profile=True, wait_for_user=True)

        client = CDPClient(cdp_url)
        client.connect()

        # Open multiple tabs
        tab_ids = []
        for url in urls:
            tab = client.create_tab(url)
            tab_ids.append(tab['id'])
            print(f"Opened tab: {url} → {tab['id']}")

        # Wait for pages to load
        time.sleep(3)

        # List all tabs (包含用户已有的标签页)
        print("\n=== All Tabs ===")
        all_tabs = client.list_tabs()
        for i, tab in enumerate(all_tabs):
            print(f"  [{i}] {tab['title']} — {tab['url']}")

        # Visit each tab we created and get info
        print("\n=== Tab Details ===")
        for tab_id in tab_ids:
            client.detach()
            client.attach(tab_id)

            snapshot = PageSnapshot(client)
            actions = BrowserActions(client, snapshot)

            title = actions.get_title()
            url = actions.get_url()
            print(f"\nTab {tab_id}:")
            print(f"  Title: {title}")
            print(f"  URL: {url}")

            # Quick accessibility snapshot
            tree = snapshot.accessibility_tree(max_depth=2, compact=True)
            # Show first 500 chars
            preview = tree[:500]
            if len(tree) > 500:
                preview += '\n  ...'
            print(f"  Snapshot:\n{preview}")

        # 关闭我们创建的标签页（不要关闭用户原有的标签页）
        print("\n=== Closing tabs we created ===")
        for tab_id in tab_ids:
            client.close_tab(tab_id)
            print(f"  Closed: {tab_id}")

    finally:
        if client:
            client.close()
        launcher.stop()
        print("Done.")


if __name__ == '__main__':
    main()
