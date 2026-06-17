# Changelog

All notable changes to the `OsirisDB` project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] - 2026-06-17

### Added

- **Buffer Pool Manager**: Implemented `BufferPool` to manage a fixed-capacity, in-memory cache of pages backed by `HeapFile` using an LRU eviction policy.
- **Storage Error Handling**: Added `StorageError::BufferPoolFull` to handle cases where all frames are pinned.
- **Integration Tests**: Added an integration test suite for verifying buffer pool pinning/unpinning, LRU eviction, dirty page flushing, and error handling.

## [0.4.0] - 2026-06-10

### Added

- **Storage Engine**: Introduced a synchronous disk-storage engine (`src/storage/`) to manage on-disk layout under a root data directory (`data_dir`).
- **Database Directory DDL**: Implemented creation and removal of database directories on disk, including automatic setup of default `public` schema directories.
- **Executor Integration**: Configured `Executor` to accept an optional `Storage` engine and added `Executor::new_in_memory` fallback for tests and benchmarks.
- **Storage Benchmarks & Tests**: Added Criterion benchmark suite (`benches/storage/`) and integration tests (`tests/executor/database_test.rs`) to verify storage behaviour.

## [0.3.0] - 2026-06-09

### Added

- **Execution Engine**: Implemented the baseline execution framework (`Executor`) to apply bound statements to the catalog.
- **Database DDL Executor**: Added support for executing `CREATE DATABASE` statements (with optional owner, encoding, locale, tablespace, connection limits, and `IF NOT EXISTS` handling).
- **Integration Tests**: Added an integration test suite under `tests/executor/` to verify executor correctness, catalog state persistence, and OID increments.
- **Performance Benchmarks**: Added Criterion benchmark suite under `benches/executor/` to measure catalog-size impact on binding and execution speeds.

## [0.2.0] - 2026-06-07

### Added

- **Testing Framework**: Initialized project-wide testing. Relocated integration tests from `src/tests` to the root-level `tests/` directory so they are automatically run by Cargo.
- **Benchmarking & Profiling**: Integrated the `criterion` benchmarking crate as a dev-dependency and configured catalog, lexer, and parser benchmarks with custom profiling harnesses.

### Fixed

- **Doctest Maintenance**: Removed illustrative test code blocks from doc comments in favor of the dedicated integration test suites.
- **AST Consistency**: Renamed the `Statement::CreateDataBase` variant to `CreateDatabase` in `src/ast/statement.rs` and `src/parser/ddl/create.rs` to fix spelling consistency.
- **Lexer Test Harness**: Fixed the `lex` helper in the lexer integration tests to correctly scan up to `TokenKind::Eof`.

## [0.1.0] - 2026-05-31

### Added

- **Hand-written Lexer**: Implemented byte-based tokenizer with standard SQL keywords and nested block comment support.
- **Recursive-Descent Parser**: Modular parser utilizing Rust extension traits.
- **Pratt Operator Precedence Parser**: Successfully parses complex SQL expressions with correct operator precedence.
- **Structured AST**: Built comprehensive representation of queries, DDL, and DML.
- **PostgreSQL DDL Support**:
  - `CREATE TABLE` (parameterized data types, constraints, default values, tablespace, ON COMMIT actions).
  - `CREATE INDEX` (CONCURRENTLY, UNIQUE, index columns, sorting order, null ordering, include list, filter expressions).
  - `CREATE VIEW` (OR REPLACE, TEMPORARY, recursive, materialized, custom columns, options, WITH CHECK OPTION).
  - `CREATE SCHEMA` (authorization, IF NOT EXISTS).
  - `CREATE SEQUENCE` (AS type, START WITH, INCREMENT BY, bounds, cache, cycle, ownership).
  - `DROP TABLE` (IF EXISTS, multiple targets, cascade/restrict behavior).
  - `TRUNCATE TABLE` (multiple tables, cascade/restrict, restart/continue identity).
  - `CREATE TYPE` (ENUM, composite/row types, RANGE, custom BASE types, and DOMAIN definitions with validation constraints and defaults).
  - `CREATE DATABASE` (IF NOT EXISTS, owner, encoding, locale, tablespace, connection limit).
  - `CREATE ROLE/USER` (IF NOT EXISTS, LOGIN, PASSWORD, SUPERUSER, CREATEDB, CREATEROLE, INHERIT, REPLICATION, CONNECTION LIMIT, VALID UNTIL, IN ROLE, ROLE).
  - `CREATE TABLESPACE` (OWNER, LOCATION, IF NOT EXISTS).
  - `CREATE EXTENSION` (IF NOT EXISTS, SCHEMA, VERSION, CASCADE).
  - `CREATE TRIGGER` (CONSTRAINT, BEFORE/AFTER/INSTEAD OF timing, INSERT/UPDATE/DELETE/TRUNCATE events, REFERENCING transition tables, FOR EACH ROW/STATEMENT level, conditional WHEN clause, execute function/procedure, and custom extensions: PRIORITY, TAGS, ENABLED/DISABLED).
  - `CREATE FUNCTION` (OR REPLACE, IF NOT EXISTS, parameters with modes [IN/OUT/INOUT/VARIADIC] and default values, returns clause [SETOF, TABLE, VOID, TRIGGER], languages [SQL, PL/pgSQL, PL/Python, PL/Perl, PL/Tcl, custom], volatility [VOLATILE/STABLE/IMMUTABLE], null behavior [STRICT/CALLED ON NULL INPUT], security definer/invoker, parallel safety, planner parameters [COST, ROWS], GUC variables [SET], and custom extensions: upfront ACCESS visibility, RAISES exceptions declaration, and clean BEGIN...END procedural block bodies or dollar-quoted bodies).
  - `CREATE PROCEDURE` (OR REPLACE, IF NOT EXISTS, parameters with modes [IN/OUT/INOUT/VARIADIC] and default values, languages [SQL, PL/pgSQL, PL/Python, PL/Perl, PL/Tcl, custom], security definer/invoker, GUC variables [SET], and custom extensions: TRANSACTION CONTROL declaration, TIMEOUT execution limit, IDEMPOTENT marker, RETRIES auto-retry count, ACCESS visibility, RAISES exceptions declaration, and dollar-quoted or BEGIN...END bodies).
- **PostgreSQL DML / Query Support**:
  - `SELECT` queries with CTE (WITH), joins, wildcards, distinct, aggregates (group by/having), sorting (order by), paging (limit/offset/fetch), and set operations (UNION/INTERSECT/EXCEPT).
  - SQL literal value tracking.
- **Library API**: Added crate-level access by refactoring module hierarchy to support integration in external projects.
- **Open-source Scaffolding**: Added Apache-2.0 License, CONTRIBUTING guidelines, detailed Architecture blueprints, and README examples.
