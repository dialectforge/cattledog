# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

libs = ["fitz", "pdfplumber", "PyPDF2", "pypdf", "pdfminer", "pikepdf"]
for lib in libs:
    try:
        __import__(lib)
        print(f"{lib}: YES")
    except ImportError:
        print(f"{lib}: no")
