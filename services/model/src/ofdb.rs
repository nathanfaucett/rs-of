pub const MIGRATION: &str = concat!(
    include_str!("../migrations/0001_init.up.sql"),
    "\n",
    include_str!("../migrations/storage_catalog.sql")
);
pub const STORAGE_CATALOG_SCHEMA: &str = include_str!("../migrations/storage_catalog.sql");
