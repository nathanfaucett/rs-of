#[cfg(feature = "ofdb")]
use idp_service::security_sync::public_security_tables;
use of_model::id::Id;
use ofdb_sql::Value;
use ofdb_sql::{SyncCandidateView, SyncRowView as CandidateRow, SyncTableView as CandidateTable};

pub struct JoinCandidate<'a> {
    pub member_id: Id,
    pub device_id: Id,
    pub endpoint: &'a str,
    pub owner: Id,
    pub name: &'a str,
}

pub struct PeerBinding {
    pub member: Id,
    pub endpoint: String,
    pub owner: Id,
}

pub fn validate(view: &SyncCandidateView) -> Result<(), String> {
    for table in &view.tables {
        let public = public_security_tables()
            .into_iter()
            .find(|public| public.name() == table.name)
            .ok_or_else(invalid)?;
        if table.columns.len() != public.columns().len()
            || table
                .columns
                .iter()
                .any(|column| !public.columns().contains(&column.as_str()))
        {
            return Err(invalid());
        }
        for row in &table.rows {
            if row.deleted {
                continue;
            }
            if row.row.values.len() != table.columns.len() {
                return Err(invalid());
            }
            for (column, value) in table.columns.iter().zip(&row.row.values) {
                if !public.permits_historical_value(column, value) {
                    return Err(format!(
                        "invalid public security historical value: {}.{column}",
                        table.name
                    ));
                }
            }
            for (column, values) in &row.conflicts {
                if values
                    .iter()
                    .any(|value| !public.permits_historical_value(column, value))
                {
                    return Err("invalid public security historical value".into());
                }
            }
        }
    }
    Ok(())
}

fn field<'a>(table: &CandidateTable, row: &'a CandidateRow, column: &str) -> Option<&'a Value> {
    if row.deleted || row.conflicts.iter().any(|(name, _)| name == column) {
        return None;
    }
    table
        .columns
        .iter()
        .position(|name| name == column)
        .and_then(|index| row.row.values.get(index))
}

fn uuid(table: &CandidateTable, row: &CandidateRow, column: &str) -> Option<Id> {
    match field(table, row, column) {
        Some(Value::Uuid(id)) => Some(*id),
        _ => None,
    }
}

fn text<'a>(table: &CandidateTable, row: &'a CandidateRow, column: &str) -> Option<&'a str> {
    match field(table, row, column) {
        Some(Value::Text(text)) => Some(text),
        _ => None,
    }
}

fn validate_device(
    table: &CandidateTable,
    row: &CandidateRow,
    bindings: &[PeerBinding],
) -> Result<(), String> {
    let owner = text(table, row, "owner_subject")
        .map(|owner| owner.parse::<Id>().map_err(|_| invalid()))
        .transpose()?;
    if owner.is_some_and(|id| id.is_nil()) {
        return Err(invalid());
    }
    if let Some(binding) = bindings.iter().find(|binding| binding.member == row.id)
        && (text(table, row, "public_key").is_some_and(|endpoint| endpoint != binding.endpoint)
            || owner.is_some_and(|owner| owner != binding.owner))
    {
        return Err(invalid());
    }
    Ok(())
}

pub fn permits_device_value(table: &str, column: &str, value: &Value) -> bool {
    if table == "device_selection_denials" {
        return match (column, value) {
            (_, Value::Null) => true, // Schema defaults are not complete rows.
            ("id", Value::Uuid(id)) => rfc_uuid(id, 7),
            ("selection_id", Value::Uuid(id)) => rfc_uuid(id, 5),
            _ => false,
        };
    }
    if matches!(
        table,
        "device_selection_policies" | "device_resource_selections"
    ) {
        match (column, value) {
            ("id", Value::Uuid(id)) => return rfc_uuid(id, 5),
            ("device_id" | "application_id" | "selected_id", Value::Uuid(id)) => {
                return !id.is_nil();
            }
            ("selected_kind", Value::Text(kind)) => {
                return matches!(kind.as_str(), "database" | "filesystem");
            }
            ("admin_allowed" | "selected", Value::Bool(_)) => return true,
            ("device_key", Value::Text(key)) => {
                return Id::parse_str(key).is_ok_and(|id| !id.is_nil());
            }
            _ => {}
        }
    }
    match (column, value) {
        (_, Value::Null) => true,
        ("id", Value::Uuid(id)) => !id.is_nil() && id.get_version_num() == 7,
        ("id", Value::Text(id)) => {
            Id::parse_str(id).is_ok_and(|id| !id.is_nil() && id.get_version_num() == 7)
        }
        (
            "state" | "approved_at" | "revoked_at" | "created_at" | "updated_at",
            Value::Integer(_),
        ) => true,
        ("owner_subject" | "name" | "public_key" | "address", Value::Text(_)) => true,
        _ => false,
    }
}

fn rfc_uuid(id: &Id, version: usize) -> bool {
    id.get_version_num() == version && id.as_bytes()[8] & 0xc0 == 0x80
}

pub fn validate_device_history(
    view: &SyncCandidateView,
    bindings: &[PeerBinding],
    approval: Option<&JoinCandidate<'_>>,
) -> Result<(), String> {
    for table in &view.tables {
        let columns =
            crate::security_sync::public_device_columns(&table.name).ok_or_else(invalid)?;
        if columns.len() != table.columns.len()
            || table
                .columns
                .iter()
                .any(|column| !columns.contains(&column.as_str()))
        {
            return Err(invalid());
        }
        if table.name == "device_selection_denials" {
            for row in table.rows.iter().filter(|row| !row.deleted) {
                if !row.conflicts.is_empty()
                    || !rfc_uuid(&row.id, 7)
                    || uuid(table, row, "id") != Some(row.id)
                    || !uuid(table, row, "selection_id").is_some_and(|id| rfc_uuid(&id, 5))
                    || row.row.values.len() != table.columns.len()
                {
                    return Err(invalid());
                }
            }
        }
        if table.name != "devices" {
            continue;
        }
        for row in &table.rows {
            if row.deleted {
                continue;
            }
            if row.row.values.len() != table.columns.len()
                || table
                    .columns
                    .iter()
                    .zip(&row.row.values)
                    .any(|(column, value)| {
                        !columns.contains(&column.as_str())
                            || !permits_device_value(&table.name, column, value)
                    })
                || row.conflicts.iter().any(|(column, values)| {
                    !columns.contains(&column.as_str())
                        || values
                            .iter()
                            .any(|value| !permits_device_value(&table.name, column, value))
                })
            {
                return Err(invalid());
            }
            validate_device(table, row, bindings)?;
            if let Some(approval) = approval
                && row.id == approval.device_id
                && text(table, row, "name").is_some_and(|name| name != approval.name)
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

fn invalid() -> String {
    "invalid public security candidate".into()
}
