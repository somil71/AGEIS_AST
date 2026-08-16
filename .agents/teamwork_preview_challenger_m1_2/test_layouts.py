import asyncio
from playwright.async_api import async_playwright
import json
import http.server
import socketserver
import threading
import os
import sys

PORT = 8081
DIRECTORY = r"d:\NEEDLE\src\assets"

class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=DIRECTORY, **kwargs)
    
    def do_GET(self):
        if self.path == "/api/status":
            self.send_response(200)
            self.send_header("Content-type", "application/json")
            self.end_headers()
            self.wfile.write(b"{}")
        elif self.path == "/api/files":
            self.send_response(200)
            self.send_header("Content-type", "application/json")
            self.end_headers()
            self.wfile.write(b"{}")
        elif self.path == "/api/todos":
            self.send_response(200)
            self.send_header("Content-type", "application/json")
            self.end_headers()
            self.wfile.write(b"{}")
        elif self.path == "/api/graph":
            self.send_response(200)
            self.send_header("Content-type", "application/json")
            self.end_headers()
            graph_data = {
                "nodes": [
                    {"id": "1", "name": "Node 1", "kind": "module", "file_path": "/src/a"},
                    {"id": "2", "name": "Node 2", "kind": "module", "file_path": "/src/b"},
                    {"id": "3", "name": "Node 3", "kind": "module", "file_path": "/src/c"}
                ],
                "edges": [
                    {"from": "1", "to": "2", "kind": "calls"},
                    {"from": "2", "to": "3", "kind": "calls"},
                    {"from": "3", "to": "1", "kind": "calls"}
                ]
            }
            self.wfile.write(json.dumps(graph_data).encode())
        else:
            super().do_GET()

def start_server():
    with socketserver.TCPServer(("", PORT), Handler) as httpd:
        httpd.serve_forever()

server_thread = threading.Thread(target=start_server, daemon=True)
server_thread.start()

async def main():
    try:
        async with async_playwright() as p:
            browser = await p.chromium.launch(headless=True)
            page = await browser.new_page()
            
            print("Navigating to ui.html...")
            await page.goto(f"http://localhost:{PORT}/ui.html#graph")
            await page.wait_for_timeout(1000)
            
            # Flow Layout
            print("Switching to Flow layout...")
            await page.evaluate("setGraphLayout(document.querySelector('[data-view=\"flow\"]'))")
            await page.wait_for_timeout(1000)
            rects = await page.locator("rect").count()
            print(f"Flow layout rects: {rects}")
            if rects < 3:
                print("Error: Flow layout did not render correctly")
            
            # Bundle Layout
            print("Switching to Bundle layout...")
            await page.evaluate("setGraphLayout(document.querySelector('[data-view=\"bundle\"]'))")
            await page.wait_for_timeout(1000)
            paths = await page.locator("path").count()
            print(f"Bundle layout paths: {paths}")
            if paths < 1:
                print("Error: Bundle layout did not render correctly")
                
            # Tree Layout
            print("Switching to Tree layout...")
            await page.evaluate("setGraphLayout(document.querySelector('[data-view=\"tree\"]'))")
            await page.wait_for_timeout(1000)
            treenodes = await page.locator(".treenode").count()
            print(f"Tree layout nodes: {treenodes}")
            if treenodes < 1:
                print("Error: Tree layout did not render correctly")
                
            # Block (Treemap) Layout
            print("Switching to Block layout...")
            await page.evaluate("setGraphLayout(document.querySelector('[data-view=\"block\"]'))")
            await page.wait_for_timeout(1000)
            block_rects = await page.locator("rect").count()
            print(f"Block layout rects: {block_rects}")
            if block_rects < 1:
                print("Error: Block layout did not render correctly")
                
            await browser.close()
            print("Tests finished successfully.")
    except Exception as e:
        print(f"Exception: {e}")
        sys.exit(1)

asyncio.run(main())
