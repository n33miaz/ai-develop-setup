use crate::{
    StorageError,
    open::{APPLICATION_ID, SCHEMA_VERSION},
};
use rusqlite::{Connection, TransactionBehavior};

pub(crate) const META_SQL: &str =
    "CREATE TABLE meta (key TEXT PRIMARY KEY, value BLOB NOT NULL) STRICT, WITHOUT ROWID";

pub(crate) fn migrate(connection: &mut Connection) -> Result<(), StorageError> {
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 0 {
        tx.execute_batch(META_SQL)?;
        tx.pragma_update(None, "application_id", APPLICATION_ID)?;
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    } else if version != SCHEMA_VERSION {
        return Err(StorageError::UnsupportedSchema);
    }
    tx.commit()?;
    Ok(())
}
