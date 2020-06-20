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
//! # conn.iterate(&sql, |_| { true }).unwrap();
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
//! conn.execute(&stmt).unwrap();
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
//! conn.iterate(&sql, |pairs| {
//!     for &(column, value) in pairs.iter() {
//!         println!("{} = {}", column, value.unwrap());
//!     }
//!     true
//! }).unwrap();
//! ```
//!
//! It can be executed after getting all the rows of the query:
//!
//! ```
//! # let mut conn = owsql::sqlite::open(":memory:").unwrap();
//! # let stmt = conn.ow(r#"CREATE TABLE users (name TEXT, age INTEGER);
//! #               INSERT INTO users (name, age) VALUES ('Alice', 42);
//! #               INSERT INTO users (name, age) VALUES ('Bob', 69);"#);
//! # conn.execute(stmt).unwrap();
//! let age = "50";
//! let sql = conn.ow("SELECT * FROM users WHERE age > ") + age;
//! let rows = conn.rows(&sql).unwrap();
//! for row in rows.iter() {
//!     println!("name = {}", row.get("name").unwrap_or("NULL"));
//! }
//! ```


mod bidimap;
pub mod error;

#[cfg(feature = "sqlite")]
pub mod sqlite;

/// A typedef of the result returned by many methods.
pub type Result<T, E = crate::error::OwsqlError> = std::result::Result<T, E>;

/// This macro is a convenient way to pass named parameters to a statement.
///
/// ```
/// # use owsql::params;
/// # let mut conn = owsql::sqlite::open(":memory:").unwrap();
/// let alice = "Alice";
/// let sql = conn.add_allowlist( params![ alice, "Bob" ] );
/// ```
#[macro_export]
macro_rules! params {
    ( $( $param:expr ),* ) => {
        {
            let mut temp_vec = Vec::new();
            $(
                #[cfg(feature = "sqlite")]
                temp_vec.push($crate::sqlite::value::Value::from($param));
            )*
            temp_vec
        }
    };
}

