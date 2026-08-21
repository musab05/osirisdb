# Changelog

All notable changes to the `OsirisDB` project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Phase 3: Write-Ahead Logging (WAL) & Recovery Infrastructure**:
  - **Physiological LogRecord**:
    - Implemented `LogRecord` supporting physiological record types (`Insert`, `Delete`, `Update`, `Begin`, `Commit`, `Abort`, `CheckpointBegin`, `CheckpointEnd`, `Compensation`).
    - Added binary serialization and deserialization with CRC32C integrity checksum verification.
    - Added before/after image byte slices, slot offset, and tuple length tracking.
  - **Thread-Safe LogManager & Group Commit**:
    - Created `LogManager` and `LogManagerInner` with atomic, lock-free LSN generation using `AtomicU64`.
    - Implemented background flusher thread (`flusher_loop`) batching pending records with periodic 5ms `fsync` intervals.
    - Implemented Group Commit condition variable signaling via `wait_for_flush(target_lsn)` to wake up committing transaction threads simultaneously without redundant `fsync` calls.
  - **Buffer Pool & TableHeap WAL Protocol Integration**:
    - Connected `BufferPool` to `LogManager` via `BufferPool::with_log_manager`.
    - Enforced the fundamental Write-Ahead Logging invariant in `BufferPool::evict` and `BufferPool::flush_all`: dirty pages are prevented from writing to disk until `page.page_lsn() <= log_manager.get_flushed_lsn()`.
    - Updated `TableHeap` (`TableHeap::open_with_log_manager`) to emit `Insert` `LogRecord`s and update `page.set_page_lsn(lsn.0)` on tuple insertion.
  - **Test Suite & Benchmarks**:
    - Added comprehensive unit and concurrency test suite in `tests/storage/log_manager_test.rs` (concurrent appenders across 8 threads, parallel group commit waiters, capacity auto-flush).
    - Added WAL protocol enforcement tests in `tests/storage/database_test.rs` (`table_heap_with_log_manager_assigns_page_lsn`, `buffer_pool_eviction_enforces_wal_flush_rule`, `buffer_pool_flush_all_enforces_wal_flush_rule`).
    - Added Criterion benchmarks in `benches/storage/log_manager_bench.rs` and `benches/storage/database_bench.rs` comparing tuple insertion throughput with and without WAL logging.

## [0.8.0] - 2026-08-07

### Added

- **Storage Engine Enhancements**:
  - Implemented slotted page format (`TablePage`, `PageHeader`) replacing raw page management for better space utilization.
  - Added TOAST (The Oversized-Attribute Storage Technique) support for serializing and deserializing large, out-of-line tuples.
  - Introduced page compaction and `vacuum` methods in `TableHeap` to reclaim fragmented free space.
  - Replaced FNV-1a hashing with CRC32C checksums for robust data integrity verification across all storage pages.
  - Enhanced page persistence with Write-Ahead Logging (WAL) and added `write_page_durable` for immediate `fsync` support.

## [0.7.0] - 2026-07-20

### Added

- **B+Tree Indexing & On-Disk Index Page Storage**:
  - Implemented `BPlusTreeIndex` supporting lookup, insert, delete, and recursive page split/merge deletion algorithms.
  - Introduced `IndexPage` for index page management with structural header layout (including `free_space_pointer` / FSP, header size of 9 bytes, page free list, and deallocation).
  - Added page underflow handling, index page missing errors, and storage directory validation checks.
  - Coupled catalog `TableEntry` with `TableHeap` and `BPlusTreeIndex` storage backends.
- **Write-Ahead Logging (WAL) & Crash Consistency**:
  - Implemented Write-Ahead Logging (`WAL` module) for crash consistency and page modification recovery.
  - Added WAL logging support to `HeapFile` page allocation and storage operations for durable data updates.
- **SQL Session Management & `USE` Statement**:
  - Added full pipeline support for `USE <database>;` SQL statements (`UseDatabaseStmt`).
  - Integrated `USE` statement support into lexer, parser, AST, binder, and session module (`database_session`).
- **SQL Execution & Query Pipeline**:
  - Implemented equality predicate filtering in `SELECT` statement `WHERE` clauses (`WHERE col = val`).
  - Added `sql` and `execution` top-level modules for executing raw SQL command strings and managing end-to-end statement execution workflows.
- **CASE Expression Binder & Evaluator**:
  - Added compile-time scalar evaluation and binding for simple and searched `CASE` expressions in `eval_expr`.
  - Added comprehensive binder integration tests for `CASE` expressions (`tests/binder/expr_test.rs`).
- **Runtime Constraint Enforcement**:
  - Added runtime primary key and uniqueness constraint enforcement in `execute_insert_table` via sequential table heap scans (supporting single-column and composite key constraints).
  - Added integration tests for INSERT constraint violations (`tests/executor/constraint_test.rs`).
- **Buffer Pool & Storage Primitives**:
  - Added `frame_to_page` reverse mapping in `BufferPool` for efficient frame lookups and page lifecycle management.
  - Introduced `RawPage` struct for handling raw byte array page storage.
  - Exposed `as_bytes_mut` on `Page` for direct buffer modifications.

### Changed & Refactored

- **Storage Layer Modularization**:
  - Reorganized `src/storage` into modular subdirectories (`btree`, `file`, `heap`, `page`, `pool`, `ddl`).
  - Fixed page eviction bug in `BufferPool` and `HeapFile::write_page` to resolve page IDs using the buffer pool `page_table` rather than reading corrupted bytes from index page headers.
- **Thread-Safety & Memory Safety**:
  - Updated `BufferPool` handles in `TableHeap` and `SystemCatalog` to `Arc<Mutex<BufferPool>>` for thread-safe concurrent access.
  - Refactored `Interner` with `get_or_intern` for thread-safe string interning.
- **Error Handling & Insert Optimizations**:
  - Updated `RecordId::from_bytes` to return `Result<RecordId, StorageError>` for safer byte decoding error handling.
  - Optimized `INSERT` execution key validation and reused pre-serialized keys.
  - Enhanced table constraint handling and validation in Binder.
- **Documentation & Benchmarks**:
  - Added 1000-row INSERT execution performance test suite (`tests/executor/insert_test.rs`) and Criterion benchmarks (`benches/executor/insert_bench.rs`).
  - Moved architecture diagrams and execution lifecycle documentation from `README.md` to `ARCHITECTURE.md`.



### Added

- **Expression Binder & Evaluator**:
  - Implemented a bind-time scalar expression evaluator (`eval_expr`) supporting literals, unary operators (`+`, `-`, `NOT`), binary operators (arithmetic, comparison, logical, string concatenation), `IS NULL`, `BETWEEN`, and `IN` list validation.
  - Integrated support for standard functions: `now()`, `current_timestamp()`, `current_date()`, `upper()`, `lower()`, and `coalesce()`.
  - Introduced `BindError::DivisionByZero` and improved diagnostic details on `BindError::TypeMismatch`.
- **SQL INSERT Support (DML)**:
  - Added query parsing for SQL `INSERT` statements with support for `VALUES`, `SELECT`, and `ON CONFLICT` clauses.
  - Implemented insert binder logic, target column resolution, and constraint checking.
  - Added strict bind-time type validation, range-checking (e.g., for `SMALLINT`), and implicit integer-to-float widening.
  - Implemented insert execution with persistent table heap writing to store tuple pages on disk.
- **Query Execution (SELECT)**:
  - Implemented basic `SELECT * FROM table_name` binding and execution logic to retrieve records from table heap files.
- **System Catalog Persistence & Recovery**:
  - Added disk persistence and recovery for `SystemCatalog` metadata (database, schema, and table definitions).
  - Integrated system catalog writing into DDL executors for database, schema, and table creation.
  - Enabled automatic restoration of the in-memory catalog state from system tables on database startup.
  - Enabled dynamic OID initialization on startup as `max(existing OIDs) + 1` to prevent ID collisions.
- **On-Disk Storage Engine**:
  - Structured storage directories under a root `data_dir` to organize databases, schemas, and tables.
  - Configured the buffer pool manager to automatically flush all dirty frames to disk on database shutdown (`Drop` implementation).
  - Added extensive integration tests for storage, tuple serialization, and dirty eviction behavior.
- **New Types and Aliases**:
  - Added keyword support for SQL/PostgreSQL type aliases including `INT2`, `INT4`, `INT8`, `BOOL`, `CHARACTER`, `FLOAT4`, `FLOAT8`, `VARBINARY`, `TIMESTAMPTZ`.
  - Added AST representations for `Interval` and `Bytea` types.

### Changed & Optimized

- **Binder Performance**:
  - Pre-built index lookup maps (`col_map`) to optimize column name checks from O(N) to O(1).
  - Moved NOT NULL and primary key validation out of row iteration.
  - Pre-evaluated literal `DEFAULT` expressions once per statement rather than per row.
- **Parser Optimizations**:
  - Streamlined parser expression evaluation by passing LHS by move and reading binding power by reference.
  - Optimized parser `parse_data_type` to match on token variants directly, eliminating string allocations and case conversions.
- **Refactoring**:
  - Renamed query table AST module from `query/table.rs` to `query/table_ref.rs`.

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
