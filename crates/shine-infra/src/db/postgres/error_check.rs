use tokio_postgres::error::SqlState;

pub trait PgErrorChecks {
    fn is_constraint(&self, table: &str, constraint: &str) -> bool;
    fn is_raise_exception(&self, message: &str) -> bool;
}

impl PgErrorChecks for tokio_postgres::Error {
    fn is_constraint(&self, table: &str, constraint: &str) -> bool {
        // Match the violated constraint by its exact name from the error's constraint field, not a
        // substring of the message, so e.g. `user_email` no longer matches `user_email_key`.
        let Some(err) = self.as_db_error() else {
            return false;
        };
        matches!(
            *err.code(),
            SqlState::UNIQUE_VIOLATION | SqlState::FOREIGN_KEY_VIOLATION | SqlState::CHECK_VIOLATION
        ) && err.table() == Some(table)
            && err.constraint() == Some(constraint)
    }

    fn is_raise_exception(&self, message: &str) -> bool {
        if let Some(err) = self.as_db_error() {
            if &SqlState::RAISE_EXCEPTION == err.code() && err.message().contains(message) {
                return true;
            }
        }
        false
    }
}
