use crate::ast::*;

#[derive(Debug, Clone, PartialEq)]
pub struct CreateViewStmt {
    pub or_replace: bool,
    pub temporary: bool,
    pub recursive: bool,
    pub name: Vec<String>,
    pub columns: Vec<String>,
    pub with_options: Vec<SqlOption>,
    pub query: Box<SelectStmt>,
    pub check_option: Option<ViewCheckOption>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropViewStmt {
    pub if_exist: bool,
    pub names: Vec<ObjectName>,
    pub behaviour: Option<DropBehavior>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterViewStmt {
    pub name: ObjectName,
    pub action: AlterViewAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterViewAction {
    Rename(String),
    SetOwner(String),
    SetSchema(String),
    SetOptions(Vec<SqlOption>),
    ResetOptions(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewCheckOption {
    Local,
    Cascaded,
}