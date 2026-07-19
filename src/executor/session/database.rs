use crate::{
    binder::bound::database::BoundUseDatabaseStmt,
    executor::{ExecutionError, Executor},
};

impl Executor {
    /// Executes a `USE` statement by updating the active session database tracking.
    ///
    /// Note: The binder has already guaranteed that this database exists in the catalog,
    /// so the executor can safely apply it without double-checking.
    pub fn execute_use_database(
        &mut self,
        stmt: BoundUseDatabaseStmt,
    ) -> Result<(), ExecutionError> {
        // Mutate the session state inside the executor
        self.current_database = Some(stmt.database_name);

        Ok(())
    }
}
