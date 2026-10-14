# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

try:
    import pdfplumber
    print("pdfplumber OK")
except ImportError:
    print("NEED pdfplumber")
try:
    import PyPDF2
    print("PyPDF2 OK")
except ImportError:
    print("NEED PyPDF2")
