# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

"""
Lu Ai — Desktop Shortcut Creator

Creates a Windows desktop shortcut for the Lu Ai release build
and converts the logo PNG to a Windows .ico file for the icon.

Usage:
    python create_shortcut.py

Prerequisites:
    pip install Pillow pywin32
    Save lu_ai_logo.png to <AI_FACTORY_ROOT>\assets\
"""

import os
import sys

ASSETS = r"<AI_FACTORY_ROOT>\assets"
LOGO_PNG = os.path.join(ASSETS, "lu_ai_logo.png")
LOGO_ICO = os.path.join(ASSETS, "lu_ai.ico")
TARGET_EXE = r"<AI_FACTORY_ROOT>\target\release\lu_ai.exe"
WORKING_DIR = r"<AI_FACTORY_ROOT>"
DESKTOP = os.path.join(os.path.expanduser("~"), "Desktop")
SHORTCUT_PATH = os.path.join(DESKTOP, "Lu Ai.lnk")


def convert_png_to_ico():
    """Convert the logo PNG to a multi-size Windows .ico file."""
    from PIL import Image

    if not os.path.exists(LOGO_PNG):
        print(f"ERROR: Logo not found at {LOGO_PNG}")
        print("Save your Lu Ai logo as lu_ai_logo.png in the assets folder first.")
        return False

    img = Image.open(LOGO_PNG).convert("RGBA")

    # Create multiple sizes for crisp display at different DPIs
    sizes = [(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    icons = []
    for size in sizes:
        resized = img.resize(size, Image.LANCZOS)
        icons.append(resized)

    icons[0].save(LOGO_ICO, format="ICO", sizes=[(s, s) for _, s in enumerate(sizes)],
                  append_images=icons[1:])
    print(f"Icon created: {LOGO_ICO}")
    return True


def create_shortcut():
    """Create a Windows desktop shortcut for Lu Ai."""
    try:
        import win32com.client
    except ImportError:
        print("ERROR: pywin32 not installed. Install with: pip install pywin32")
        return False

    shell = win32com.client.Dispatch("WScript.Shell")
    shortcut = shell.CreateShortCut(SHORTCUT_PATH)
    shortcut.TargetPath = TARGET_EXE
    shortcut.WorkingDirectory = WORKING_DIR
    shortcut.Description = "Lu Ai — Associative Memory Terminal"

    if os.path.exists(LOGO_ICO):
        shortcut.IconLocation = LOGO_ICO
        print(f"Icon set: {LOGO_ICO}")

    shortcut.save()
    print(f"Shortcut created: {SHORTCUT_PATH}")
    return True


if __name__ == "__main__":
    print("=" * 45)
    print("  Lu Ai — Desktop Shortcut Creator")
    print("=" * 45)
    print()

    # Step 1: Convert logo
    has_icon = convert_png_to_ico()

    # Step 2: Check exe exists
    if not os.path.exists(TARGET_EXE):
        print(f"\nWARNING: Release exe not found at {TARGET_EXE}")
        print("Run 'cargo build --release' first, or the shortcut will point to a missing file.")

    # Step 3: Create shortcut
    create_shortcut()

    if not has_icon:
        print("\nNote: Shortcut created without custom icon.")
        print(f"To add the icon later, save lu_ai_logo.png to {ASSETS}")
        print("and run this script again.")
