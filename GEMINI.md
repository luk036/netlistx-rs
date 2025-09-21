# GEMINI.md: Your AI Assistant's Guide to this Project

This file provides context for the Gemini AI assistant to help you with this project.

## Project Overview

This is a Rust project named `netlistx-rs`. Based on the file contents, it appears to be a library for creating and manipulating netlists, which are data structures used in electronic design automation. The project is in an early stage of development.

The core data structure is the `Netlist` struct defined in `src/netlist.rs`. It uses the `petgraph` crate to represent the underlying graph. The library also includes a module for rational trigonometry (`src/trigonom.rs`).

## Building and Running

This is a Rust library project. The following commands are standard for building and testing:

*   **Build:** `cargo build`
*   **Run tests:** `cargo test`
*   **Check for errors:** `cargo check`

## Development Conventions

*   The project follows standard Rust conventions.
*   The code is licensed under both the MIT and Apache-2.0 licenses.
*   Contributions are welcome, as indicated by the `CONTRIBUTING.md` file.
