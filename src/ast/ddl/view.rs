use crate::ast::*;
use crate::common::symbol::Symbol;

/// Represents a DDL statement to construct a virtual table query representation (`CREATE VIEW`).
#[derive(Debug, Clone, PartialEq)]
pub struct CreateViewStmt {
    /// Overwrite view definition if it already exists (`OR REPLACE`).
    pub or_replace: bool,
    /// If `true`, the view exists only for the duration of the current session (`TEMPORARY` or `TEMP`).
    pub temporary: bool,
    /// If `true`, represents a recursive query representation (`RECURSIVE`).
    pub recursive: bool,
    /// The qualified name of the view.
    pub name: ObjectName,
    /// Optional column aliases defined for the view projection.
    pub columns: Vec<Symbol>,
    /// View storage parameters (`WITH`).
    pub with_options: Vec<SqlOption>,
    /// The SELECT query that defines the view contents.
    pub query: Box<SelectStmt>,
    /// Optional CHECK OPTION to prevent rows from being modified if they do not match the view's query.
    pub check_option: Option<ViewCheckOption>,
}

/// Represents a DDL statement to delete views (`DROP VIEW`).
#[derive(Debug, Clone, PartialEq)]
pub struct DropViewStmt {
    /// Skip error creation if the view does not exist (`IF EXISTS`).
    pub if_exist: bool,
    /// The names of the views to drop.
    pub names: Vec<ObjectName>,
    /// Cascade or Restrict deletion behavior (`CASCADE` / `RESTRICT`).
    pub behaviour: Option<DropBehavior>,
}

/// Represents a DDL statement to alter views (`ALTER VIEW`).
#[derive(Debug, Clone, PartialEq)]
pub struct AlterViewStmt {
    /// The view being altered.
    pub name: ObjectName,
    /// The action to perform.
    pub action: AlterViewAction,
}

/// Action alternatives for `ALTER VIEW`.
#[derive(Debug, Clone, PartialEq)]
pub enum AlterViewAction {
    /// Rename the view.
    Rename(Symbol),
    /// Set a new owner for the view.
    SetOwner(Symbol),
    /// Move the view to a different schema.
    SetSchema(Symbol),
    /// Set specific view options.
    SetOptions(Vec<SqlOption>),
    /// Reset/Remove specific view options.
    ResetOptions(Vec<Symbol>),
}

/// Verification options for values inserted/updated via the view (`WITH CHECK OPTION`).
#[derive(Debug, Clone, PartialEq)]
pub enum ViewCheckOption {
    /// Check applies only to the immediate view's conditions (`LOCAL`).
    Local,
    /// Check applies to the view and all underlying nested views (`CASCADED`).
    Cascaded,
}
