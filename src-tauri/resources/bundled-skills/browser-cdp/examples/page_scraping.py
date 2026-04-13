#!/usr/bin/env python3
"""
Example: Page Scraping — 复用用户浏览器提取页面结构化数据。

典型场景：抓取需要登录的页面（如小红书、知乎），利用用户已有的 session。

Usage:
    python examples/page_scraping.py [URL]
"""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / 'scripts'))

from browser_launcher import BrowserLauncher
from cdp_client import CDPClient
from page_snapshot import PageSnapshot
from browser_actions import BrowserActions


def main():
    url = sys.argv[1] if len(sys.argv) > 1 else 'https://news.ycombinator.com'

    launcher = BrowserLauncher()
    client = None

    try:
        # 复用用户已有的 Chrome（保留登录态，可抓取需要登录的页面）
        print("Connecting to user's browser...")
        cdp_url = launcher.launch(browser='chrome', reuse_profile=True, wait_for_user=True)

        client = CDPClient(cdp_url)
        client.connect()

        # 创建新标签页进行抓取，不影响用户正在浏览的页面
        tab = client.create_tab(url)
        client.attach(tab['id'])

        snapshot = PageSnapshot(client)
        actions = BrowserActions(client, snapshot)

        # Navigate
        print(f"Navigating to {url}...")
        actions.navigate(url)

        # Method 1: Accessibility tree (quick overview)
        print("\n=== Accessibility Tree ===")
        tree = snapshot.accessibility_tree(max_depth=4, compact=True)
        print(tree[:3000])  # First 3000 chars

        # Method 2: DOM snapshot
        print("\n=== DOM Snapshot ===")
        dom = snapshot.dom_snapshot(selector='body', max_depth=4)
        print(dom[:3000])

        # Method 3: Extract structured data via JS
        print("\n=== Structured Data (via JS) ===")
        data = actions.evaluate('''
            (function() {
                // Extract all links on the page
                const links = Array.from(document.querySelectorAll('a[href]')).map(a => ({
                    text: a.textContent.trim().substring(0, 100),
                    href: a.href,
                })).filter(l => l.text && l.href.startsWith('http'));

                // Extract all headings
                const headings = Array.from(document.querySelectorAll('h1,h2,h3')).map(h => ({
                    level: h.tagName,
                    text: h.textContent.trim(),
                }));

                // Extract meta info
                const title = document.title;
                const description = document.querySelector('meta[name="description"]')?.content || '';

                return { title, description, headings, links: links.slice(0, 20) };
            })()
        ''')

        print(json.dumps(data, indent=2, ensure_ascii=False))

        # Method 4: Get plain text content
        print("\n=== Page Text (first 1000 chars) ===")
        text = snapshot.get_text()
        print(text[:1000])

        # Save full extraction to file
        output_path = '/tmp/scraping_result.json'
        with open(output_path, 'w', encoding='utf-8') as f:
            json.dump(data, f, indent=2, ensure_ascii=False)
        print(f"\nFull extraction saved to: {output_path}")

    finally:
        if client:
            client.close()
        launcher.stop()
        print("Done.")


if __name__ == '__main__':
    main()
