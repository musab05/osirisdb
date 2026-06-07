/// A lightweight, copy-able identifier for interned strings.
///
/// # What is a Symbol?
///
/// A `Symbol` is a numeric handle that represents a string stored in an
/// [`Interner`]. Instead of passing `String` or `&str` around the entire
/// compiler/database pipeline, we pass a `Symbol` — a single `u32`.
///
/// This matters because:
/// - `String` is 24 bytes on the stack + heap allocation
/// - `Symbol` is 4 bytes on the stack, no heap allocation
/// - Comparing two `Symbol`s is a single integer comparison (`u32 == u32`)
/// - Comparing two `String`s requires scanning every character
///
/// # Why interning?
///
/// In a SQL engine, the same identifier appears many times:
///
/// ```sql
/// SELECT users.id, users.name, users.email
/// FROM users
/// WHERE users.id = 1
/// ```
///
/// Without interning: `"users"` allocates a new `String` 4 times.
/// With interning: `"users"` is stored once, referenced as `Symbol(3)` everywhere.
///
/// This means:
/// - Zero redundant heap allocations for repeated identifiers
/// - O(1) equality checks instead of O(n) string scans
/// - Smaller AST nodes, catalog entries, and plan nodes
/// - Better cache locality — plan nodes fit in fewer cache lines
///
/// # How it works
///
/// ```text
/// "users"  ─── intern() ──→  Symbol(0)
/// "id"     ─── intern() ──→  Symbol(1)
/// "name"   ─── intern() ──→  Symbol(2)
/// "users"  ─── intern() ──→  Symbol(0)  ← same symbol, no new allocation
/// ```
///
/// To get the string back: `interner.resolve(Symbol(0))` → `"users"`
///
/// # Usage in the pipeline
///
/// Every layer of the engine uses `Symbol` instead of `String` for names:
///
/// ```text
/// Parser       → produces Symbol from source &str via Interner
/// AST          → stores Symbol in ColumnDef, TableRef, FunctionParam etc.
/// Binder       → looks up Symbol in Catalog, compares Symbols for resolution
/// Logical Plan → Symbol in column references, table references
/// Optimizer    → Symbol comparisons for predicate pushdown, projection pruning
/// Executor     → Symbol for column lookup in row batches
/// Catalog      → Symbol as HashMap keys for O(1) table/column lookup
/// ```
///
/// # Properties
///
/// - [`Copy`] — passed by value everywhere, no `.clone()` needed
/// - [`Eq`] + [`Hash`] — usable as `HashMap` keys directly
/// - [`PartialEq`] — `sym_a == sym_b` is a single integer comparison
///

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Symbol(pub u32);

impl Symbol {
    /// A sentinel value representing an unresolved or missing symbol.
    ///
    /// Used as a safe placeholder before a name has been interned or
    /// when a name is optional and not provided. Using `u32::MAX` ensures
    /// it can never collide with a real interned symbol since no interner
    /// will ever hold 4 billion strings.
    ///
    /// # When to use DUMMY
    ///
    /// - Optional parameter names before resolution: `name: Option<Symbol>` is
    ///   preferred, but `DUMMY` can be useful in fixed-size structs where
    ///   `Option` adds overhead.
    /// - Placeholder during multi-pass construction where a name is filled
    ///   in on a second pass.
    /// - Unit tests where a real symbol is not needed.
    ///
    /// # Warning
    ///
    /// Never pass `DUMMY` to `interner.resolve()` — it will panic with an
    /// out-of-bounds index. Always check before resolving:

    pub const DUMMY: Symbol = Symbol(u32::MAX);

    /// Returns `true` if this symbol is the dummy sentinel value.
    ///
    /// Convenience method to avoid comparing against `Symbol::DUMMY` directly.

    pub fn is_dummy(&self) -> bool {
        self.0 == u32::MAX
    }

    /// Returns the raw numeric id of this symbol.
    ///
    /// Useful for serialization, debugging, or storing in compact
    /// data structures. The id is stable within a single interner
    /// instance but not across different interners or restarts.

    pub fn id(&self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for Symbol {
    /// For human-readable output, resolve the symbol through the interner.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Symbol({})", self.0)
    }
}
