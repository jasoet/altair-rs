//! Convenience re-exports — one `use altair_db::prelude::*;` is enough
//! to write straightforward CRUD against the database.

pub use crate::{Backend, Config, Db, Error, Result};

pub use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
};
