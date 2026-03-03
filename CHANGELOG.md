# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Comprehensive error handling with `Result<T, E>` returns for all operations
- Builder pattern for constructing netlists with fluent API
- Partitioning algorithms: Fiduccia-Mattheyses and Kernighan-Lin
- Statistics and metrics module for netlist analysis
- File I/O support for ISPD and Verilog formats
- O(1) lookups using `IndexMap` for module and net indices
- Optional serde support for serialization/deserialization
- Optional rayon support for parallel processing
- Property-based tests using quickcheck
- Integration tests
- Benchmark suite using criterion
- Cached degree calculations for performance optimization
- Methods for getting connected modules/nets
- `has_module()` and `has_net()` helper methods

### Changed
- Improved type safety with proper error types
- Optimized data structures using `IndexMap` and `IndexSet`
- Updated `num_modules` and `num_nets` to methods instead of fields
- Fixed graph type to use proper undirected graph API
- Improved code organization with separate modules for different functionalities

### Fixed
- Duplicate module tests causing compilation errors
- Warnings reported by clippy
- Doc comment formatting issues
- Method call syntax for new API

### Removed
- Old test module that was causing duplication

## [0.1.2] - Previous Release
- Initial release with basic netlist functionality
