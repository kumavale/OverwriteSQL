use std::collections::HashSet;
use std::cell::RefCell;
use std::fmt;

use crate::Result;
use crate::OwsqlConn;
use crate::bidimap::BidiMap;
use crate::error::{OwsqlError, OwsqlErrorLevel};
use crate::constants::OW_MINIMUM_LENGTH;
use crate::overwrite::{IntoInner, overwrite_new};
use crate::serial::SerialNumber;
use crate::parser::*;
use crate::row::Row;

pub(crate) enum DBType {
    Sqlite,
    Mysql,
    Postgresql,
}

/// A database connection.
pub struct Connection {
    pub(crate) conn:          Box<dyn OwsqlConn>,
    pub(crate) allowlist:     HashSet<String>,
    pub(crate) serial_number: RefCell<SerialNumber>,
    pub(crate) ow_len_range:  (usize, usize),
    pub(crate) overwrite:     RefCell<BidiMap<String, String>>,
    pub(crate) error_msg:     RefCell<BidiMap<OwsqlError, String>>,
    pub(crate) error_level:   OwsqlErrorLevel,
}

unsafe impl Send for Connection {}
unsafe impl Sync for Connection {}

impl PartialEq for Connection {
    fn eq(&self, other: &Self) -> bool {
        (&self.conn as *const _) == (&other.conn as *const _)
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("conn", &(&self.conn as *const _))
            .field("ow_len_range", &self.ow_len_range)
            .field("error_level", &self.error_level)
            .finish()
    }
}

impl Connection {
    /// Execute a statement without processing the resulting rows if any.
    ///
    /// # Examples
    ///
    /// ```
    /// # let mut conn = owsql::sqlite::open(":memory:").unwrap();
    /// # let stmt = conn.ow(r#"CREATE TABLE users (name TEXT, id INTEGER);
    /// #               INSERT INTO users (name, id) VALUES ('Alice', 42);
    /// #               INSERT INTO users (name, id) VALUES ('Bob', 69);"#);
    /// # conn.execute(stmt).unwrap();
    /// let sql = conn.ow(r#"SELECT * FROM users;"#);
    /// conn.execute(&sql).unwrap();
    /// ```
    #[inline]
    pub fn execute<T: AsRef<str>>(&self, query: T) -> Result<()> {
        self.conn._execute(self.convert_to_valid_syntax(query.as_ref()), &self.error_level)
    }

    /// Execute a statement and process the resulting rows as plain text.
    ///
    /// The callback is triggered for each row. If the callback returns `false`,
    /// no more rows will be processed.
    ///
    /// # Examples
    ///
    /// ```
    /// # let mut conn = owsql::sqlite::open(":memory:").unwrap();
    /// # let stmt = conn.ow(r#"CREATE TABLE users (name TEXT, id INTEGER);
    /// #               INSERT INTO users (name, id) VALUES ('Alice', 42);
    /// #               INSERT INTO users (name, id) VALUES ('Bob', 69);"#);
    /// # conn.execute(stmt).unwrap();
    /// let sql = conn.ow(r#"SELECT * FROM users;"#);
    /// conn.iterate(&sql, |pairs| {
    ///     for &(column, value) in pairs.iter() {
    ///         println!("{} = {}", column, value.unwrap());
    ///     }
    ///     true
    /// }).unwrap();
    /// ```
    #[inline]
    pub fn iterate<T: AsRef<str>, F>(&self, query: T, mut callback: F) -> Result<()>
        where
            F: FnMut(&[(&str, Option<&str>)]) -> bool,
    {
        self.conn._iterate(self.convert_to_valid_syntax(query.as_ref()), &self.error_level, &mut callback)
    }

    /// Execute a statement and returns the rows.
    ///
    /// # Examples
    ///
    /// ```
    /// # let mut conn = owsql::sqlite::open(":memory:").unwrap();
    /// # let stmt = conn.ow(r#"CREATE TABLE users (name TEXT, id INTEGER);
    /// #               INSERT INTO users (name, id) VALUES ('Alice', 42);
    /// #               INSERT INTO users (name, id) VALUES ('Bob', 69);"#);
    /// # conn.execute(stmt).unwrap();
    /// let sql = conn.ow(r#"SELECT name FROM users;"#);
    /// let rows = conn.rows(&sql).unwrap();
    /// for row in rows.iter() {
    ///     println!("name: {}", row.get("name").unwrap_or("NULL"));
    /// }
    /// ```
    #[inline]
    pub fn rows<T: AsRef<str>>(&self, query: T) -> Result<Vec<Row>> {
        let mut rows: Vec<Row> = Vec::new();

        self.iterate(query, |pairs| {
            let mut row = Row::new();
            for (column, value) in pairs.iter() {
                row.insert(column.to_string(), value.as_ref().map(|v| v.to_string()));
            }
            rows.push(row);
            true
        })?;

        Ok(rows)
    }

    /// Return the actual SQL statement.
    ///
    /// # Examples
    ///
    /// ```
    /// # use owsql::error::OwsqlError;
    /// let mut conn = owsql::sqlite::open(":memory:").unwrap();
    /// let select = conn.ow("SELECT");
    /// let oreilly = conn.ow("O'Reilly");
    /// let oreilly_unhtmlescape = unsafe { conn.ow_without_html_escape("O'Reilly") };
    /// assert_eq!(conn.actual_sql(&select).unwrap(), "SELECT ");
    /// assert_eq!(conn.actual_sql("SELECT").unwrap(), "'SELECT' ");
    /// assert_eq!(conn.actual_sql(&oreilly), Err(OwsqlError::Message("invalid literal".to_string())));
    /// assert_eq!(conn.actual_sql("O'Reilly").unwrap(), "'O&#39;Reilly' ");
    /// assert_eq!(conn.actual_sql(&oreilly_unhtmlescape).unwrap(), "'O''Reilly' ");
    /// ```
    #[inline]
    pub fn actual_sql<T: AsRef<str>>(&self, query: T) -> Result<String> {
        self.convert_to_valid_syntax(query.as_ref())
    }

    /// Return the overwrite definition string.  
    /// All strings assembled without using this method are escaped.  
    /// This method does not sanitize.  
    /// A string containing incomplete quotes like the one below will result in an error.  
    ///
    /// # Errors
    ///
    /// ```rust
    /// # let mut conn = owsql::sqlite::open(":memory:").unwrap();
    /// # let name = "bar";
    /// conn.ow("where name = 'foo' OR name = '") + name + &conn.ow("';");
    /// # /*
    ///                                       ^                      ^
    /// # */
    /// ```
    ///
    /// # Examples
    ///
    /// ```
    /// # let mut conn = owsql::sqlite::open(":memory:").unwrap();
    /// let sql = conn.ow("SELECT");
    ///
    /// assert_eq!(sql, conn.ow("SELECT"));
    /// assert_ne!(sql, "SELECT");
    /// ```
    #[inline]
    pub fn ow<T: ?Sized + std::string::ToString>(&self, s: &'static T) -> String {
        let s = s.to_string();
        let result = self.check_valid_literal(&s);
        match result {
            Ok(_) => {
                if !self.overwrite.borrow_mut().contain(&s) {
                    let overwrite = overwrite_new(self.serial_number.borrow_mut().get(), self.ow_len_range);
                    self.overwrite.borrow_mut().insert(s.to_string(), overwrite);
                }
                format!(" {} ", self.overwrite.borrow_mut().get(&s).unwrap())
            },
            Err(e) => {
                if !self.error_msg.borrow_mut().contain(&e) {
                    let overwrite = overwrite_new(self.serial_number.borrow_mut().get(), self.ow_len_range);
                    self.error_msg.borrow_mut().insert(e.clone(), overwrite);
                }
                format!(" {} ", self.error_msg.borrow_mut().get(&e).unwrap())
            },
        }
    }

    /// Return the overwrite definition string without HTML escape.  
    ///
    /// # Safety
    ///
    /// This is an unsafe method!! => I am considering whether to use the unsafe keyword :(  
    /// Note that this can be XSS.
    #[inline]
    pub unsafe fn ow_without_html_escape<T: Clone + ToString>(&self, value: T) -> String {
        let s = match self.conn.db_type() {
            DBType::Sqlite => format!("'{}'", single_quotaion_escape(&value.to_string())),
            _ => format!("'{}'", single_quotaion_and_backslash_escape(&value.to_string())),
        };
        let result = self.check_valid_literal(&s);
        match result {
            Ok(_) => {
                if !self.overwrite.borrow_mut().contain(&s) {
                    let overwrite = overwrite_new(self.serial_number.borrow_mut().get(), self.ow_len_range);
                    self.overwrite.borrow_mut().insert(s.to_string(), overwrite);
                }
                format!(" {} ", self.overwrite.borrow_mut().get(&s).unwrap())
            },
            Err(e) => {
                if !self.error_msg.borrow_mut().contain(&e) {
                    let overwrite = overwrite_new(self.serial_number.borrow_mut().get(), self.ow_len_range);
                    self.error_msg.borrow_mut().insert(e.clone(), overwrite);
                }
                format!(" {} ", self.error_msg.borrow_mut().get(&e).unwrap())
            },
        }
    }

    /// Return the overwrite definition string in allowlist.  
    /// Returns the escaped string.  
    ///
    /// # Examples
    ///
    /// ```
    /// use owsql::params;
    /// # let mut conn = owsql::sqlite::open(":memory:").unwrap();
    /// # let stmt = conn.ow(r#"CREATE TABLE users (name TEXT, id INTEGER);
    /// #               INSERT INTO users (name, id) VALUES ('Alice', 42);
    /// #               INSERT INTO users (name, id) VALUES ('Bob', 69);"#);
    /// # conn.execute(stmt).unwrap();
    /// conn.add_allowlist(params!["Alice", "Bob"]);
    /// let input = "Alice OR 1=1; --";
    /// let sql = conn.ow("SELECT * FROM users WHERE name = ") + &conn.allowlist(input);
    ///
    /// assert!(conn.execute(sql).is_err());
    /// ```
    #[inline]
    pub fn allowlist<T: Clone + ToString>(&self, value: T) -> String {
        if self.is_allowlist(value.clone()) {
            format!(" {} ", self.overwrite.borrow_mut().get(&escape_for_allowlist(&value.to_string())).unwrap())
        } else {
            let e = OwsqlError::new(&self.error_level, "deny value", &value.to_string()).err().unwrap_or(OwsqlError::AnyError);
            if !self.error_msg.borrow_mut().contain(&e) {
                let overwrite = overwrite_new(self.serial_number.borrow_mut().get(), self.ow_len_range);
                self.error_msg.borrow_mut().insert(e.clone(), overwrite);
            }
            format!(" {} ", self.error_msg.borrow_mut().get(&e).unwrap())
        }
    }

    /// Checks if the value is within the allowlist.
    ///
    /// # Examples
    ///
    /// ```
    /// use owsql::params;
    /// # let mut conn = owsql::sqlite::open(":memory:").unwrap();
    /// conn.add_allowlist(params!["Alice", "Bob", 42, 123]);
    /// assert!(conn.is_allowlist("Alice"));
    /// assert!(!conn.is_allowlist("'Alice'"));
    /// assert!(conn.is_allowlist(42));
    /// assert!(conn.is_allowlist("42"));
    /// assert!(!conn.is_allowlist("'42'"));
    /// ```
    #[inline]
    pub fn is_allowlist<T: ToString>(&self, value: T) -> bool {
        self.allowlist.contains(&value.to_string())
    }

    /// Register it in self.overwrite after performing character string escape processing with
    /// single quotation added to both sides.  
    /// Use [params macro](../macro.params.html).  
    ///
    /// # Examples
    ///
    /// ```
    /// use owsql::params;
    /// # let mut conn = owsql::sqlite::open(":memory:").unwrap();
    /// conn.add_allowlist(params!["Alice", 'A', 42, 0.123]);
    /// ```
    #[inline]
    pub fn add_allowlist(&mut self, params: Vec<crate::value::Value>) {
    //pub fn add_allowlist(&mut self, params: &[&(dyn ToString + Sync)]) {
        for value in params {
            self.allowlist.insert(value.to_string());
            self.overwrite.borrow_mut().insert(
                escape_for_allowlist(&value.to_string()),
                overwrite_new(self.serial_number.borrow_mut().get(), self.ow_len_range)
            );
        }
    }

    /// It is guaranteed to be a signed 64-bit integer without quotation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use owsql::params;
    /// # let mut conn = owsql::sqlite::open(":memory:").unwrap();
    /// conn.int(42);              // ok
    /// conn.int("42");            // ok
    /// conn.int("42 or 1=1; --"); // error
    /// ```
    #[inline]
    pub fn int<T: Clone + ToString>(&self, value: T) -> String {
        let value = value.to_string();
        if value.parse::<i64>().is_ok() {
            if !self.overwrite.borrow_mut().contain(&value) {
                let overwrite = overwrite_new(self.serial_number.borrow_mut().get(), self.ow_len_range);
                self.overwrite.borrow_mut().insert(value.to_string(), overwrite);
            }
            format!(" {} ", self.overwrite.borrow_mut().get(&value).unwrap())
        } else {
            let e = OwsqlError::new(&self.error_level, "non integer", &value).err().unwrap_or(OwsqlError::AnyError);
            if !self.error_msg.borrow_mut().contain(&e) {
                let overwrite = overwrite_new(self.serial_number.borrow_mut().get(), self.ow_len_range);
                self.error_msg.borrow_mut().insert(e.clone(), overwrite);
            }
            format!(" {} ", self.error_msg.borrow_mut().get(&e).unwrap())
        }
    }

    /// You can set a different fixed value or a different length each time.  
    /// The [ow method](./struct.SqliteConnection.html#method.ow) outputs a random number of about 32
    /// digits by default.  
    /// However, if a number less than 32 digits is entered, it will be set to 32 digits.  
    ///
    /// # Examples
    ///
    /// ```
    /// # use owsql::params;
    /// # let mut conn = owsql::sqlite::open(":memory:").unwrap();
    /// conn.set_ow_len(42);       // 42
    /// conn.set_ow_len(50..100);  // 50-99
    /// conn.set_ow_len(50..=100); // 50-100
    /// ```
    #[inline]
    pub fn set_ow_len<T: 'static + IntoInner>(&mut self, range: T) {
        self.ow_len_range = {
            let range = range.into_inner();
            let range0 = if range.0 < OW_MINIMUM_LENGTH { OW_MINIMUM_LENGTH } else { range.0 };
            let range1 = if range.1 < OW_MINIMUM_LENGTH { OW_MINIMUM_LENGTH } else { range.1 };
            (range0, range1)
        };
    }

    /// Sets the error level.  
    /// The default value is [OwsqlErrorLevel](../error/enum.OwsqlErrorLevel.html)::Develop for debug builds and [OwsqlErrorLevel](../error/enum.OwsqlErrorLevel.html)::Release for release builds.
    ///
    /// # Examples
    ///
    /// ```
    /// # use owsql::error::OwsqlErrorLevel;
    /// # let mut conn = owsql::sqlite::open(":memory:").unwrap();
    /// conn.error_level(OwsqlErrorLevel::Debug).unwrap();
    /// ```
    #[inline]
    pub fn error_level(&mut self, level: OwsqlErrorLevel) -> Result<(), &str> {
        if cfg!(not(debug_assertions)) && level == OwsqlErrorLevel::Debug {
            return Err("OwsqlErrorLevel::Debug cannot be set during release build");
        }
        self.error_level = level;
        Ok(())
    }
}
