use core::fmt;

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[derive(serde::Deserialize)]
pub struct StorageQuery {
    pub access_token: String,
    pub filesystem_id: String,
}

impl fmt::Debug for StorageQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StorageQuery")
            .field("filesystem_id", &self.filesystem_id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    use alloc::format;

    use super::StorageQuery;

    #[test]
    fn storage_query_debug_omits_the_access_token() {
        let query = StorageQuery {
            access_token: "storage-query-secret".into(),
            filesystem_id: "filesystem-context".into(),
        };
        let output = format!("{query:?}");

        assert!(output.contains("StorageQuery"));
        assert!(output.contains("filesystem-context"));
        assert!(!output.contains("storage-query-secret"));
        assert!(!output.contains("access_token"));
    }
}
