use crate::common::symbol::Symbol;
use std::collections::HashMap;

/// A string interner that maps strings to compact numeric [`Symbol`]s.
///
/// # What is an Interner?
///
/// An `Interner` is a data structure that ensures each unique string is
/// stored exactly once in memory and assigned a unique [`Symbol`] id.
/// All subsequent uses of the same string return the same `Symbol` with
/// zero additional allocation.
///
/// # Why do we need this?
///
/// A SQL engine sees the same identifiers repeatedly across every layer:
///
/// ```sql
/// SELECT users.id, users.name FROM users WHERE users.active = true
/// ```
///
/// Without interning — `"users"` allocates 4 separate `String`s on the heap.
/// With interning — `"users"` is allocated once, referenced as `Symbol(0)`
/// everywhere. The remaining 3 references cost nothing.
///
/// Beyond allocation savings, interning enables:
///
/// - **O(1) equality** — `Symbol(0) == Symbol(0)` is one integer compare
/// - **HashMap keys** — `HashMap<Symbol, T>` is faster than `HashMap<String, T>`
/// - **Smaller nodes** — AST, plan, and catalog nodes hold a `u32` not a `String`
/// - **Cache efficiency** — more data fits in CPU cache lines
///
/// # Memory model
///
/// Interned strings are intentionally leaked via [`Box::leak`]. This gives
/// them a `'static` lifetime, meaning they live for the entire duration of
/// the process. This is a deliberate trade-off:
///
/// - **Pro** — no lifetime parameters needed anywhere in the engine
/// - **Pro** — no reference counting overhead
/// - **Con** — memory is never freed (acceptable for a database process
///   that typically runs until shutdown)
///
/// If you need to reclaim memory (e.g. in tests or short-lived processes),
/// consider using an arena allocator like `bumpalo` instead of `Box::leak`.
///
/// # Internal structure
///
/// ```text
/// strings: Vec<&'static str>         symbols: HashMap<&'static str, Symbol>
/// ┌─────────────────────────┐        ┌──────────────────────────────────┐
/// │ [0] "users"             │        │ "users"  → Symbol(0)             │
/// │ [1] "id"                │        │ "id"     → Symbol(1)             │
/// │ [2] "name"              │        │ "name"   → Symbol(2)             │
/// │ [3] "orders"            │        │ "orders" → Symbol(3)             │
/// └─────────────────────────┘        └──────────────────────────────────┘
///      ↑ resolve(Symbol(2))                ↑ intern("name") → Symbol(2)
///      returns "name"
/// ```
///
/// # Thread safety
///
/// `Interner` is **not** thread-safe. For concurrent query execution, wrap
/// it in `Arc<RwLock<Interner>>` or use a per-thread interner with a shared
/// read-only view after the parse phase.

pub struct Interner {
    /// Maps interned string slices to their assigned [`Symbol`] id.
    ///
    /// The keys are `&'static str` obtained via [`Box::leak`] so they
    /// share the same pointer as the corresponding entry in `strings`.
    /// This avoids storing the string data twice.
    map: HashMap<&'static str, Symbol>,

    /// Stores all interned strings indexed by their [`Symbol`] id.
    ///
    /// `strings[sym.0]` gives the string for `sym`.
    /// This is the reverse direction of `map` and enables O(1) resolution.
    strings: Vec<&'static str>,
}

impl Interner {
    /// Creates a new empty `Interner`.
    ///
    /// No strings are pre-interned. The first call to [`intern`] will
    /// assign `Symbol(0)`, the second `Symbol(1)`, and so on.
    
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            strings: Vec::new(),
        }
    }

    /// Interns a string and returns its [`Symbol`].
    ///
    /// If the string has been interned before, the existing [`Symbol`] is
    /// returned immediately with **zero allocation**. If it is new, the
    /// string is leaked onto the heap, stored, and a fresh [`Symbol`] is
    /// assigned.
    ///
    /// # Allocation behavior
    ///
    /// ```text
    /// First call:  intern("users") → allocates, returns Symbol(0)
    /// Second call: intern("users") → no allocation, returns Symbol(0)
    /// Third call:  intern("id")    → allocates, returns Symbol(1)
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if more than `u32::MAX - 1` unique strings are interned.
    /// In practice this limit (≈4 billion) will never be reached.
    
    pub fn intern(&mut self, s: &str) -> Symbol {
        // Fast path — string already interned, return existing symbol.
        // This is the common case in a SQL engine where identifiers repeat.
        if let Some(&sym) = self.map.get(s) {
            return sym;
        }

        // Slow path — new string, assign next available symbol id.
        let id = Symbol(self.strings.len() as u32);

        // Leak the string to obtain a 'static lifetime.
        // This is intentional — interned strings live for the process lifetime.
        // The memory is never freed, which is acceptable for a long-running
        // database process. See struct-level docs for alternatives.
        let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());

        // Store in both directions for O(1) lookup in either direction.
        self.strings.push(leaked); // Symbol → &str  (resolve)
        self.map.insert(leaked, id); // &str → Symbol  (intern)

        id
    }

    /// Resolves a [`Symbol`] back to its original string.
    ///
    /// This is the reverse of [`intern`]. The returned `&str` has a
    /// `'static` lifetime because interned strings are leaked onto the heap
    /// and never freed.
    ///
    /// # Panics
    ///
    /// Panics if `sym` was not produced by this interner, or if
    /// `sym` is [`Symbol::DUMMY`] (`Symbol(u32::MAX)`).
    
    pub fn resolve(&self, sym: Symbol) -> &str {
        self.strings[sym.0 as usize]
    }

    /// Looks up a string without interning it.
    ///
    /// Returns `Some(Symbol)` if the string has already been interned,
    /// or `None` if it has not. Unlike [`intern`], this never allocates
    /// and never modifies the interner.
    ///
    /// # When to use this vs `intern`
    ///
    /// - Use [`intern`] during **parsing** — you want to assign a symbol
    ///   regardless of whether the string is new.
    /// - Use [`get`] during **binding/resolution** — you want to check if
    ///   a name exists in the catalog without adding it. If `None` is
    ///   returned, the name is undefined and should produce an error.
    
    pub fn get(&self, s: &str) -> Option<Symbol> {
        self.map.get(s).copied()
    }

    /// Returns the number of unique strings currently interned.
    ///
    /// Useful for diagnostics, testing, and capacity planning.
    
    pub fn len(&self) -> usize {
        self.strings.len()
    }

    /// Returns `true` if no strings have been interned yet.
    
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}

impl Default for Interner {
    /// Creates a new empty `Interner`.
    ///
    /// Equivalent to [`Interner::new`]. Provided so `Interner` can be
    /// used with `#[derive(Default)]` in structs that contain it.
    fn default() -> Self {
        Self::new()
    }
}
