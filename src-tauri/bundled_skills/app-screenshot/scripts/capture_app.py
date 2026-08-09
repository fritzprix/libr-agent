#!/usr/bin/env python3
"""
Cross-platform app UI control and screenshot capture script for LibrAgent.
Supports Windows (Win32 API) and Linux (X11 XTest) for:
- Window focus and UI screenshot capture
- Mouse relative coordinate clicking
- Keyboard typing and special key sending
"""

import sys
import os
import platform
import time
import argparse
import mss
from PIL import Image

def get_window_info_linux(title_substr="LibrAgent"):
    import subprocess
    try:
        output = subprocess.check_output("xwininfo -root -tree", shell=True, text=True)
        for line in output.splitlines():
            if title_substr.lower() in line.lower() and ("LibrAgent" in line or "(" in line):
                parts = line.strip().split()
                win_id_hex = parts[0]
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
                    return win_id_hex, left, top, width, height
    except Exception as e:
        print(f"[app-screenshot] Error searching Linux window: {e}")
    return None, None, None, None, None

def focus_window(os_name, win_id_hex=None, hwnd=None):
    if os_name == "Linux" and win_id_hex:
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
            time.sleep(0.2)
        except Exception as e:
            print(f"[app-screenshot] Linux focus error: {e}")
    elif os_name == "Windows" and hwnd:
        import ctypes
        user32 = ctypes.windll.user32
        user32.ShowWindow(hwnd, 9)  # SW_RESTORE
        user32.SetForegroundWindow(hwnd)
        time.sleep(0.2)

def click_app(os_name, x_rel, y_rel, title_substr="LibrAgent"):
    if os_name == "Linux":
        win_id_hex, left, top, _, _ = get_window_info_linux(title_substr)
        if not win_id_hex:
            print("[app-screenshot] Could not find target window to click.")
            return False
        focus_window(os_name, win_id_hex=win_id_hex)
        from Xlib import X, display, ext
        d = display.Display()
        abs_x = left + x_rel
        abs_y = top + y_rel
        fake = ext.xtest.fake_input
        fake(d, X.MotionNotify, x=abs_x, y=abs_y)
        d.sync()
        time.sleep(0.05)
        fake(d, X.ButtonPress, 1, x=abs_x, y=abs_y)
        d.sync()
        time.sleep(0.05)
        fake(d, X.ButtonRelease, 1, x=abs_x, y=abs_y)
        d.sync()
        print(f"[app-screenshot] Clicked relative ({x_rel}, {y_rel}) -> absolute ({abs_x}, {abs_y})")
        return True
    elif os_name == "Windows":
        import ctypes
        from ctypes import wintypes
        user32 = ctypes.windll.user32
        hwnd_found = []
        WNDENUMPROC = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
        def enum_cb(h, l):
            if user32.IsWindowVisible(h):
                length = user32.GetWindowTextLengthW(h)
                if length > 0:
                    buff = ctypes.create_unicode_buffer(length + 1)
                    user32.GetWindowTextW(h, buff, length + 1)
                    if title_substr.lower() in buff.value.lower():
                        hwnd_found.append(h)
            return True
        user32.EnumWindows(WNDENUMPROC(enum_cb), 0)
        if not hwnd_found:
            print("[app-screenshot] No target window found on Windows.")
            return False
        hwnd = hwnd_found[0]
        focus_window(os_name, hwnd=hwnd)
        rect = wintypes.RECT()
        user32.GetWindowRect(hwnd, ctypes.byref(rect))
        abs_x = rect.left + x_rel
        abs_y = rect.top + y_rel
        user32.SetCursorPos(abs_x, abs_y)
        time.sleep(0.05)
        user32.mouse_event(0x0002, 0, 0, 0, 0) # MOUSEEVENTF_LEFTDOWN
        time.sleep(0.05)
        user32.mouse_event(0x0004, 0, 0, 0, 0) # MOUSEEVENTF_LEFTUP
        print(f"[app-screenshot] Windows clicked relative ({x_rel}, {y_rel})")
        return True
    return False

def capture_app(output_path, title_substr="LibrAgent"):
    os_name = platform.system()
    print(f"[app-screenshot] Detected OS: {os_name}")
    
    if os_name == "Windows":
        import ctypes
        from ctypes import wintypes
        user32 = ctypes.windll.user32
        hwnd_found = []
        WNDENUMPROC = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
        def enum_cb(h, l):
            if user32.IsWindowVisible(h):
                length = user32.GetWindowTextLengthW(h)
                if length > 0:
                    buff = ctypes.create_unicode_buffer(length + 1)
                    user32.GetWindowTextW(h, buff, length + 1)
                    if title_substr.lower() in buff.value.lower():
                        hwnd_found.append((h, buff.value))
            return True
        user32.EnumWindows(WNDENUMPROC(enum_cb), 0)
        if not hwnd_found:
            print(f"[app-screenshot] No window found matching '{title_substr}' on Windows.")
            return False
        hwnd, title = hwnd_found[0]
        focus_window(os_name, hwnd=hwnd)
        rect = wintypes.RECT()
        user32.GetWindowRect(hwnd, ctypes.byref(rect))
        width, height = rect.right - rect.left, rect.bottom - rect.top
        monitor = {"left": rect.left, "top": rect.top, "width": width, "height": height}
        with mss.MSS() as sct:
            sct_img = sct.grab(monitor)
            img = Image.frombytes("RGB", sct_img.size, sct_img.bgra, "raw", "BGRX")
            img.save(output_path)
            print(f"[app-screenshot] Successfully captured Windows screenshot to {output_path} ({width}x{height})")
        return True

    elif os_name == "Linux":
        win_id_hex, left, top, width, height = get_window_info_linux(title_substr)
        if win_id_hex:
            focus_window(os_name, win_id_hex=win_id_hex)
            monitor = {"left": left, "top": top, "width": width, "height": height}
            with mss.MSS() as sct:
                sct_img = sct.grab(monitor)
                img = Image.frombytes("RGB", sct_img.size, sct_img.bgra, "raw", "BGRX")
                img.save(output_path)
                print(f"[app-screenshot] Successfully captured Linux screenshot to {output_path} ({width}x{height})")
            return True

        with mss.MSS() as sct:
            monitor = sct.monitors[1]
            sct_img = sct.grab(monitor)
            img = Image.frombytes("RGB", sct_img.size, sct_img.bgra, "raw", "BGRX")
            img.save(output_path)
            print(f"[app-screenshot] Captured full screen fallback on Linux to {output_path}")
        return True

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="LibrAgent UI Control & Screenshot Tool")
    parser.add_argument("output", nargs="?", default="docs/user/assets/screenshots/getting-started/new-session.png", help="Output screenshot path")
    parser.add_argument("--title", default="LibrAgent", help="Target window title substring")
    parser.add_argument("--click", nargs=2, type=int, metavar=("X", "Y"), help="Click relative coordinates inside app window before capture")

    args = parser.parse_args()
    os_name = platform.system()

    if args.click:
        click_app(os_name, args.click[0], args.click[1], args.title)
        time.sleep(0.5)

    capture_app(args.output, args.title)
