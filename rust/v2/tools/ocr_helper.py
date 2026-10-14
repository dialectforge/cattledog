#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""OCR helper for Lu's vision pipeline.

Usage: python ocr_helper.py <image_path>
Outputs extracted text to stdout. Exits 0 on success, 1 on error.

Tries pytesseract first, then easyocr as fallback.
Requires Tesseract to be installed for pytesseract:
  Windows: https://github.com/UB-Mannheim/tesseract/wiki
  Linux:   sudo apt install tesseract-ocr
  macOS:   brew install tesseract
"""
import sys
import os

def ocr_pytesseract(image_path):
    import pytesseract
    from PIL import Image
    img = Image.open(image_path)
    return pytesseract.image_to_string(img)

def ocr_easyocr(image_path):
    import easyocr
    reader = easyocr.Reader(['en'], verbose=False)
    results = reader.readtext(image_path, detail=0)
    return "\n".join(results)

def main():
    if len(sys.argv) < 2:
        print("Usage: ocr_helper.py <image_path>", file=sys.stderr)
        sys.exit(1)

    image_path = sys.argv[1]
    if not os.path.isfile(image_path):
        print(f"File not found: {image_path}", file=sys.stderr)
        sys.exit(1)

    # Try pytesseract first
    try:
        text = ocr_pytesseract(image_path)
        text = text.strip()
        if text:
            print(text)
            return
    except ImportError:
        pass  # pytesseract not installed, try next
    except Exception as e:
        print(f"pytesseract error: {e}", file=sys.stderr)

    # Fallback to easyocr
    try:
        text = ocr_easyocr(image_path)
        text = text.strip()
        if text:
            print(text)
            return
    except ImportError:
        print("No OCR library available. Install pytesseract or easyocr.", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(f"easyocr error: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()
