# Contributing to OsirisDB

Thank you for your interest in contributing to `OsirisDB`! We welcome contributions of all forms, including bug fixes, feature implementations, documentation improvements, and bug reports.

## Code of Conduct

Please be respectful and constructive in all communication with the maintainers and other contributors.

## How to Contribute

### 1. Bug Reports and Feature Requests
Please open an issue on GitHub. When reporting a bug:
- Provide a minimal reproducible example (e.g., the SQL string that failed to parse).
- Provide the expected output or behavior versus the actual output/error.
- Include details about your environment (Rust compiler version).

### 2. Development Setup
To set up your workspace:
1. Install Rust (latest stable recommended) from [rustup.rs](https://rustup.rs).
2. Clone the repository:
   ```bash
   git clone https://github.com/musab05/osirisdb.git
   cd osirisdb
   ```
3. Build the project:
   ```bash
   cargo build
   ```
4. Run the tests:
   ```bash
   cargo test
   ```

### 3. Adding Support for New SQL Syntax
The parser utilizes Rust's extension trait pattern to keep files modular. If you are adding a new statement or syntax element:
1. **Define the AST Node**: Add appropriate structs/enums in `src/ast/` under `ddl/`, `dml/`, or `query/`. Update the root `Statement` enum in `src/ast/statement.rs`.
2. **Define New Keywords/Tokens**: If new keywords are required, add them to the `TokenKind` enum in `src/lexer/token.rs` and the mapping in `src/lexer/lookup_keyword.rs`.
3. **Implement Parser Trait/Extension**:
   - If adding a new statement type, create a corresponding parser file in `src/parser/` (e.g. `src/parser/insert.rs`) and declare it in `src/parser/mod.rs`.
   - Implement the parsing logic via a trait matching the existing pattern (e.g., `pub trait InsertParser`).
   - Wire your dispatcher into the main statement parser at `src/parser/statement.rs`.
4. **Add Unit/Integration Tests**: Verify correctness using the parser API in tests.

### 4. Code Style & Standards
- Keep code clean and well-structured.
- Format your code using `cargo fmt`:
  ```bash
  cargo fmt --all
  ```
- Run clippy to check for lints:
  ```bash
  cargo clippy --all-targets
  ```
- All public functions, structs, and enums must be documented with `///` doc comments.

### 5. Submitting a Pull Request
1. Fork the repository and create a new branch.
2. Commit your changes with descriptive commit messages.
3. Ensure all tests pass.
4. Push to your fork and submit a PR to our main repository.
