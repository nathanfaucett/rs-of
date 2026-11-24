pub const DEVICE_SCHEMA: &str = include_str!("../migrations/device.sql");
pub const MIGRATION: &str = concat!(
    include_str!("../migrations/0001_init.up.sql"),
    "\n",
    include_str!("../migrations/device.sql"),
    "\n",
    include_str!("../migrations/storage_catalog.sql")
);
pub const STORAGE_CATALOG_SCHEMA: &str = include_str!("../migrations/storage_catalog.sql");

#[cfg(test)]
mod tests {
    use super::{DEVICE_SCHEMA, MIGRATION, STORAGE_CATALOG_SCHEMA};

    #[test]
    fn unified_migration_preserves_owner_schema_order() {
        let expected = [
            include_str!("../migrations/0001_init.up.sql"),
            "\n",
            DEVICE_SCHEMA,
            "\n",
            STORAGE_CATALOG_SCHEMA,
        ]
        .concat();
        assert_eq!(MIGRATION, expected);
    }
}
