pub mod catalog;
pub mod error;
pub mod manager;
pub mod objects;
// remove the duplicate pub mod manager

pub use catalog::Catalog;
pub use error::CatalogError;
pub use manager::CatalogManager;
