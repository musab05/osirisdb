# Changelog

All notable changes to the `rust_sql` project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
- **PostgreSQL DML / Query Support**:
  - `SELECT` queries with CTE (WITH), joins, wildcards, distinct, aggregates (group by/having), sorting (order by), paging (limit/offset/fetch), and set operations (UNION/INTERSECT/EXCEPT).
  - SQL literal value tracking.
- **Library API**: Added crate-level access by refactoring module hierarchy to support integration in external projects.
- **Open-source Scaffolding**: Added Apache-2.0 License, CONTRIBUTING guidelines, detailed Architecture blueprints, and README examples.
