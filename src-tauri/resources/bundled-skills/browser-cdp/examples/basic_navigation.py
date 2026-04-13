#!/usr/bin/env python3
"""
Example: Basic Navigation — 复用用户浏览器打开页面、获取标题、截图。

典型场景：大模型操控用户已有的 Chrome，利用已登录的 session/cookies。

Usage:
    python examples/basic_navigation.py [URL]
"""

import sys
from pathlib import Path

# Add scripts dir to path
sys.path.insert(0, str(Path(__file__).parent.parent / 'scripts'))

from browser_launcher import BrowserLauncher
from cdp_client import CDPClient
from page_snapshot import PageSnapshot
from browser_actions import BrowserActions


def main():
    url = sys.argv[1] if len(sys.argv) > 1 else 'https://example.com'

    launcher = BrowserLauncher()
    client = None

    try:
        # 1. 复用用户已有的 Chrome（保留登录态、cookies、扩展等）
        #    如果 Chrome 尚未开启 CDP，会自动引导用户在
        #    chrome://inspect 中勾选 Allow，并等待授权完成。
        print("Launching browser (reuse existing profile)...")
        cdp_url = launcher.launch(browser='chrome', reuse_profile=True, wait_for_user=True)
        print(f"CDP endpoint: {cdp_url}")

        # 2. Connect CDP client
        client = CDPClient(cdp_url)
        client.connect()

        # 3. Create a new tab for navigation
        tab = client.create_tab(url)
        client.attach(tab['id'])

        # 4. Navigate
        actions = BrowserActions(client)
        print(f"Navigating to {url}...")
        actions.navigate(url)

        # 5. Get page info
        title = actions.get_title()
        current_url = actions.get_url()
        print(f"Title: {title}")
        print(f"URL: {current_url}")

        # 6. Get accessibility snapshot
        snapshot = PageSnapshot(client)
        tree = snapshot.accessibility_tree(max_depth=3)
        print(f"\nAccessibility Tree (depth=3):\n{tree}")

        # 7. Screenshot
        screenshot_path = '/tmp/basic_navigation.png'
        actions.screenshot(screenshot_path, full_page=True)
        print(f"\nScreenshot saved to: {screenshot_path}")

    finally:
        if client:
            client.close()
        # 注意：reuse_profile 模式下 stop() 只关闭 Proxy，不会关闭用户的 Chrome
        launcher.stop()
        print("Done.")


if __name__ == '__main__':
    main()
