# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

"""
Lu Ai — Icon Converter & Shortcut Updater

Converts lu_ai_logo.png to lu_ai.ico and updates
the desktop shortcut to use it.

Usage:  python set_icon.py
        (save lu_ai_logo.png to assets\ first)
"""
import os, sys
ASSETS = r"<AI_FACTORY_ROOT>\assets"
PNG = os.path.join(ASSETS, "lu_ai_logo.png")
ICO = os.path.join(ASSETS, "lu_ai.ico")
LNK = r"<USER_HOME>\OneDrive\Desktop\Lu Ai.lnk"

from PIL import Image
img = Image.open(PNG).convert("RGBA")
sizes = [(16,16),(32,32),(48,48),(64,64),(128,128),(256,256)]
imgs = [img.resize(s, Image.LANCZOS) for s in sizes]
imgs[0].save(ICO, format="ICO", append_images=imgs[1:])
print(f"Created {ICO}")

import win32com.client
sh = win32com.client.Dispatch("WScript.Shell")
sc = sh.CreateShortCut(LNK)
sc.IconLocation = ICO
sc.Save()
print(f"Updated shortcut icon: {LNK}")
