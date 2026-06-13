use crate::{catalog::CatalogManager, common::symbol::Symbol};

/// The binder — validates AST nodes against the catalog and produces
/// fully resolved bound nodes ready for the executor.
///
/// # Responsibilities
///
/// - Name resolution — confirms referenced objects exist in the catalog
/// - Default resolution — fills in omitted values (e.g. owner → session user)
/// - Semantic validation — catches errors that the parser cannot
///   (e.g. unknown role, invalid connection limit)
///
/// # What the binder does NOT do
///
/// - Modify the catalog — that is the executor's job
/// - Parse SQL — that is the parser's job
/// - Optimize — that is the planner's job
///
/// # Relationship to CatalogManager
///
/// The binder holds a shared read-only reference to the catalog.
/// Only the executor writes to the catalog via `CatalogManager`.
pub struct Binder<'c> {
    /// Read-only view of the catalog for name resolution.
    pub catalog: &'c CatalogManager,

    /// The current session user — used as default owner when OWNER is omitted.
    pub session_user: Symbol,
}

impl<'c> Binder<'c> {
    /// Creates a new `Binder` for the current session.
    ///
    /// `catalog` is borrowed read-only — the binder never modifies it.
    /// `session_user` is the symbol of the currently connected role,
    /// used to resolve ownership defaults.
    pub fn new(catalog: &'c CatalogManager, session_user: Symbol) -> Self {
        Self {
            catalog,
            session_user,
        }
    }
}
