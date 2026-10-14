# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

import sys
print("Python:", sys.executable)
print("Version:", sys.version)
try:
    import datasets
    print("datasets:", datasets.__version__)
except ImportError:
    print("datasets: NOT FOUND")
