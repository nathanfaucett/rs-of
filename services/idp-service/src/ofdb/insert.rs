use of_model::id::Id;
use ofdb_sql::{Query, QueryInsert, QueryResult, Row};

use crate::repo::{RepoError, RepoResult};

pub fn generated_insert(table: &str, mut row: Row, return_id: bool) -> Query {
    // A typed row omits the UUID primary key by leaving out the first column.
    row.values.remove(0);
    Query::Insert(QueryInsert {
        table: table.into(),
        row,
        returning: return_id.then(|| vec!["id".into()]),
    })
}

pub fn generated_id(results: &[QueryResult]) -> RepoResult<Id> {
    let value = results
        .first()
        .and_then(|result| result.rows.first())
        .and_then(|row| row.values.first())
        .ok_or_else(|| RepoError::InvalidInput("missing generated id".into()))?;
    ofdb_sql::decode(value, "id").map_err(|error| RepoError::InvalidInput(error.to_string()))
}
