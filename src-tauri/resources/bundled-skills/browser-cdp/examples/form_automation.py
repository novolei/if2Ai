#!/usr/bin/env python3
"""
Example: Form Automation — 在用户浏览器中自动填写和提交表单。

典型场景：自动化填写登录表单、搜索框，或者帮用户批量填表。
使用 reuse_profile=True 复用用户的浏览器，用户可以看到操作过程。

Usage:
    python examples/form_automation.py [LOGIN_URL]
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / 'scripts'))

from browser_launcher import BrowserLauncher
from cdp_client import CDPClient
from page_snapshot import PageSnapshot
from browser_actions import BrowserActions


def main():
    url = sys.argv[1] if len(sys.argv) > 1 else 'https://example.com/login'

    launcher = BrowserLauncher()
    client = None

    try:
        # 复用用户已有的 Chrome（用户可以看到自动化操作过程）
        print("Connecting to user's browser...")
        cdp_url = launcher.launch(browser='chrome', reuse_profile=True, wait_for_user=True)

        # Connect
        client = CDPClient(cdp_url)
        client.connect()

        # 创建新标签页操作，不影响用户正在浏览的页面
        tab = client.create_tab(url)
        client.attach(tab['id'])

        snapshot = PageSnapshot(client)
        actions = BrowserActions(client, snapshot)

        # Navigate to login page
        print(f"Navigating to {url}...")
        actions.navigate(url)

        # Get accessibility tree to understand the page
        tree = snapshot.accessibility_tree()
        print(f"\nPage structure:\n{tree}")

        # --- Strategy 1: Using CSS selectors ---
        print("\n--- Using CSS selectors ---")

        # Check if there are input fields
        has_inputs = actions.evaluate(
            'document.querySelectorAll("input").length'
        )
        print(f"Found {has_inputs} input fields")

        if has_inputs and has_inputs > 0:
            # Fill username (adapt selector to your page)
            actions.fill('input[type="text"], input[name="username"], input#username', 'testuser')
            print("Filled username")

            # Fill password
            actions.fill('input[type="password"], input[name="password"]', 'testpass')
            print("Filled password")

            # Screenshot before submit
            actions.screenshot('/tmp/form_before_submit.png')
            print("Screenshot: /tmp/form_before_submit.png")

            # Submit (click button or press Enter)
            actions.press_key('Enter')
            actions.wait(2)

            # Screenshot after submit
            actions.screenshot('/tmp/form_after_submit.png')
            print("Screenshot: /tmp/form_after_submit.png")

        # --- Strategy 2: Using ref-based interaction ---
        print("\n--- Using ref-based interaction ---")
        tree = snapshot.accessibility_tree()
        print(f"Refreshed tree:\n{tree}")

        refs = snapshot.refs
        print(f"\nAvailable refs: {list(refs.keys())}")

        # Example: click the first textbox ref
        for ref, info in refs.items():
            if info['role'] == 'textbox':
                print(f"Clicking ref {ref} ({info['role']}: {info['name']})")
                actions.click_by_ref(ref)
                actions.type_text('hello from ref!')
                break

    finally:
        if client:
            client.close()
        launcher.stop()
        print("\nDone.")


if __name__ == '__main__':
    main()
