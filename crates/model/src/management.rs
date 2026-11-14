//! WebSocket sync selection for Management Service.
//!
//! This module provides the API for managing which devices have WebSocket sync
//! enabled for specific resources. This is parallel to the Iroh mesh selection
//! but for WebSocket-based sync (browser/server apps).

use alloc::{string::String, vec::Vec};
use core::fmt::Debug;

use crate::idp::model::Id;
use serde::{Deserialize, Serialize};

#[cfg(feature = "utoipa")]
use utoipa::openapi::schema::{KnownFormat, ObjectBuilder, SchemaFormat, SchemaType, Type};

#[cfg(feature = "utoipa")]
impl utoipa::ToSchema for SyncSelection {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("SyncSelection")
    }
}

#[cfg(feature = "utoipa")]
impl utoipa::PartialSchema for SyncSelection {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::Schema> {
        let mut obj = ObjectBuilder::new()
            .property(
                "device_id",
                ObjectBuilder::new()
                    .schema_type(SchemaType::Type(Type::String))
                    .format(Some(SchemaFormat::KnownFormat(KnownFormat::Uuid)))
                    .build(),
            )
            .property(
                "resource_id",
                ObjectBuilder::new()
                    .schema_type(SchemaType::Type(Type::String))
                    .format(Some(SchemaFormat::KnownFormat(KnownFormat::Uuid)))
                    .build(),
            )
            .property(
                "application_id",
                ObjectBuilder::new()
                    .schema_type(SchemaType::Type(Type::String))
                    .format(Some(SchemaFormat::KnownFormat(KnownFormat::Uuid)))
                    .build(),
            )
            .property(
                "owner_subject",
                ObjectBuilder::new()
                    .schema_type(SchemaType::Type(Type::String))
                    .build(),
            )
            .property(
                "selected_at",
                ObjectBuilder::new()
                    .schema_type(SchemaType::Type(Type::Integer))
                    .build(),
            );

        obj = obj.required("device_id");
        obj = obj.required("resource_id");
        obj = obj.required("application_id");
        obj = obj.required("owner_subject");
        obj = obj.required("selected_at");

        utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(obj.build()))
    }
}

#[cfg(feature = "utoipa")]
impl utoipa::ToSchema for SyncSelectionRequest {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("SyncSelectionRequest")
    }
}

#[cfg(feature = "utoipa")]
impl utoipa::PartialSchema for SyncSelectionRequest {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::Schema> {
        let mut obj = ObjectBuilder::new()
            .property(
                "resource_id",
                ObjectBuilder::new()
                    .schema_type(SchemaType::Type(Type::String))
                    .format(Some(SchemaFormat::KnownFormat(KnownFormat::Uuid)))
                    .build(),
            )
            .property(
                "select",
                ObjectBuilder::new()
                    .schema_type(SchemaType::Type(Type::Boolean))
                    .build(),
            );

        obj = obj.required("resource_id");
        obj = obj.required("select");

        utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(obj.build()))
    }
}

#[cfg(feature = "utoipa")]
impl utoipa::ToSchema for SyncSelectionResponse {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("SyncSelectionResponse")
    }
}

#[cfg(feature = "utoipa")]
impl utoipa::PartialSchema for SyncSelectionResponse {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::Schema> {
        let mut obj = ObjectBuilder::new().property(
            "selections",
            ObjectBuilder::new()
                .schema_type(SchemaType::Type(Type::Array))
                .build(),
        );

        obj = obj.required("selections");

        utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(obj.build()))
    }
}

/// Sync selection record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyncSelection {
    pub device_id: Id,
    pub resource_id: Id,
    pub application_id: Id,
    pub owner_subject: String,
    pub selected_at: i64,
}

/// Request to select/deselect a resource for WebSocket sync.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncSelectionRequest {
    pub resource_id: Id,
    pub select: bool,
}

/// Response for sync selection operations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncSelectionResponse {
    pub selections: Vec<SyncSelection>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idp::model::Id;
    use alloc::string::ToString;

    #[test]
    fn sync_selection_serialization() {
        let selection = SyncSelection {
            device_id: Id::now_v7(),
            resource_id: Id::now_v7(),
            application_id: Id::now_v7(),
            owner_subject: "user-123".to_string(),
            selected_at: 1234567890,
        };
        let json = serde_json::to_string(&selection).unwrap();
        let deserialized: SyncSelection = serde_json::from_str(&json).unwrap();
        assert_eq!(selection, deserialized);
    }
}
