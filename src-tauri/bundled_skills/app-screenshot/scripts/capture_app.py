#!/usr/bin/env python3
"""
Cross-platform app screenshot capture script for LibrAgent.
Supports Linux (X11) and Windows (Win32) without external binary dependencies (uses mss + PIL).
"""

import sys
import os
import platform
import time
import mss
from PIL import Image

def capture_app(output_path, title_substr="LibrAgent"):
    os_name = platform.system()
    print(f"[app-screenshot] Detected OS: {os_name}")
    
    if os_name == "Windows":
        import ctypes
        from ctypes import wintypes

        user32 = ctypes.windll.user32
        hwnd_found = []

        WNDENUMPROC = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

        def enum_cb(hwnd, lparam):
            if user32.IsWindowVisible(hwnd):
                length = user32.GetWindowTextLengthW(hwnd)
                if length > 0:
                    buff = ctypes.create_unicode_buffer(length + 1)
                    user32.GetWindowTextW(hwnd, buff, length + 1)
                    if title_substr.lower() in buff.value.lower():
                        hwnd_found.append((hwnd, buff.value))
            return True

        user32.EnumWindows(WNDENUMPROC(enum_cb), 0)

        if not hwnd_found:
            print(f"[app-screenshot] No window found matching '{title_substr}' on Windows.")
            return False

        hwnd, title = hwnd_found[0]
        print(f"[app-screenshot] Found Windows app window: '{title}' (HWND: {hwnd})")

        # Bring window to front
        user32.ShowWindow(hwnd, 9)  # SW_RESTORE
        user32.SetForegroundWindow(hwnd)
        time.sleep(0.3)

        rect = wintypes.RECT()
        user32.GetWindowRect(hwnd, ctypes.byref(rect))
        left, top, right, bottom = rect.left, rect.top, rect.right, rect.bottom
        width, height = right - left, bottom - top

        monitor = {"left": left, "top": top, "width": width, "height": height}
        with mss.MSS() as sct:
            sct_img = sct.grab(monitor)
            img = Image.frombytes("RGB", sct_img.size, sct_img.bgra, "raw", "BGRX")
            img.save(output_path)
            print(f"[app-screenshot] Successfully captured Windows screenshot to {output_path} ({width}x{height})")
        return True

    elif os_name == "Linux":
        import subprocess
        try:
            output = subprocess.check_output("xwininfo -root -tree", shell=True, text=True)
            for line in output.splitlines():
                if title_substr.lower() in line.lower() and ("LibrAgent" in line or "(" in line):
                    parts = line.strip().split()
                    win_id_hex = parts[0]
                    
                    # Try to bring window to front via Xlib
                    try:
                        from Xlib import X, display, protocol
                        d = display.Display()
                        root = d.screen().root
                        win = d.create_resource_object("window", int(win_id_hex, 16))
                        net_active = d.intern_atom("_NET_ACTIVE_WINDOW")
                        event = protocol.event.ClientMessage(
                            window=win,
                            client_type=net_active,
                            data=(32, [1, X.CurrentTime, 0, 0, 0]),
                        )
                        root.send_event(event, event_mask=X.SubstructureRedirectMask | X.SubstructureNotifyMask)
                        d.sync()
                        time.sleep(0.3)
                    except Exception:
                        pass

                    geo_out = subprocess.check_output(f"xwininfo -id {win_id_hex}", shell=True, text=True)
                    left, top, width, height = None, None, None, None
                    for g in geo_out.splitlines():
                        if "Absolute upper-left X:" in g:
                            left = int(g.split(":")[-1].strip())
                        elif "Absolute upper-left Y:" in g:
                            top = int(g.split(":")[-1].strip())
                        elif "Width:" in g:
                            width = int(g.split(":")[-1].strip())
                        elif "Height:" in g:
                            height = int(g.split(":")[-1].strip())

                    if left is not None and top is not None and width and height:
                        monitor = {"left": left, "top": top, "width": width, "height": height}
                        with mss.MSS() as sct:
                            sct_img = sct.grab(monitor)
                            img = Image.frombytes("RGB", sct_img.size, sct_img.bgra, "raw", "BGRX")
                            img.save(output_path)
                            print(f"[app-screenshot] Successfully captured Linux screenshot to {output_path} ({width}x{height})")
                        return True
        except Exception as e:
            print(f"[app-screenshot] Linux capture error: {e}")

        # Fallback to full primary monitor
        with mss.MSS() as sct:
            monitor = sct.monitors[1]
            sct_img = sct.grab(monitor)
            img = Image.frombytes("RGB", sct_img.size, sct_img.bgra, "raw", "BGRX")
            img.save(output_path)
            print(f"[app-screenshot] Captured full screen fallback on Linux to {output_path}")
        return True

    else:
        print(f"[app-screenshot] Unsupported OS: {os_name}")
        return False

if __name__ == "__main__":
    out_file = sys.argv[1] if len(sys.argv) > 1 else "docs/user/assets/screenshots/getting-started/new-session.png"
    target_title = sys.argv[2] if len(sys.argv) > 2 else "LibrAgent"
    capture_app(out_file, target_title)
