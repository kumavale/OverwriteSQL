//! # OverwriteSQL
//! `owsql` is a secure library for PostgreSQL, MySQL and SQLite.  
//! Unlike other libraries, you can use string concatenation to prevent SQL injection.  
//!
//! ```
//! # let mut conn = owsql::sqlite::open(":memory:").unwrap();
//! # let stmt = conn.ow(r#"CREATE TABLE users (name TEXT, id INTEGER);
//! #               INSERT INTO users (name, id) VALUES ('Alice', 42);
//! #               INSERT INTO users (name, id) VALUES ('Bob', 69);"#);
//! # conn.execute(stmt).unwrap();
//! let id_input = "42 OR 1=1; --";
//! let sql = conn.ow("SELECT name FROM users WHERE id = ") + id_input;
//! // At runtime it will be transformed into a query like
//! // "SELECT name FROM users WHERE id = '42 OR 1=1; --';".
//! # conn.iterate(sql, |_| { true }).unwrap();
//! ```
//!
//! ## Example
//!
//! Open a connection of SQLite, create a table, and insert some rows:
//!
//! ```
//! let mut conn = owsql::sqlite::open(":memory:").unwrap();
//! let stmt = conn.ow(r#"CREATE TABLE users (name TEXT, age INTEGER);
//!               INSERT INTO users (name, age) VALUES ('Alice', 42);
//!               INSERT INTO users (name, age) VALUES ('Bob', 69);"#);
//! conn.execute(stmt).unwrap();
//! ```
//!
//! Select some rows and process them one by one as plain text:
//!
//! ```
//! # let mut conn = owsql::sqlite::open(":memory:").unwrap();
//! # let stmt = conn.ow(r#"CREATE TABLE users (name TEXT, age INTEGER);
//! #               INSERT INTO users (name, age) VALUES ('Alice', 42);
//! #               INSERT INTO users (name, age) VALUES ('Bob', 69);"#);
//! # conn.execute(stmt).unwrap();
//! let age = "50";
//! let sql = conn.ow("SELECT * FROM users WHERE age > ") + age;
//! conn.iterate(sql, |pairs| {
//!     for &(column, value) in pairs.iter() {
//!         println!("{} = {}", column, value.unwrap());
//!     }
//!     true
//! }).unwrap();
//! ```


mod bidimap;

#[cfg(feature = "sqlite")]
pub mod sqlite;

/// Enum listing possible errors from owsql.
#[derive(Debug, PartialEq)]
pub enum OwsqlError {
    /// The error code.
    Code(isize),
    /// The error message.
    Message(String),
    /// The empty tuple like error.
    Err(()),
}

impl std::string::ToString for OwsqlError {
    fn to_string(&self) -> String {
        match self {
            OwsqlError::Code(i) =>    i.to_string(),
            OwsqlError::Message(s) => s.to_string(),
            OwsqlError::Err(()) =>    String::new(),
        }
    }
}

impl From::<()> for OwsqlError {
    fn from(_: ()) -> Self {
        OwsqlError::Err(())
    }
}

/// A typedef of the result returned by many methods.
pub type Result<T, E = OwsqlError> = std::result::Result<T, E>;

