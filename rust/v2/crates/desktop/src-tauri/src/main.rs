// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
// Prevents an additional console window on Windows in release mode.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    lu_desktop_lib::run();
}
