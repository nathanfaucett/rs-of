use std::{
    env, fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use futures::executor::block_on;

use ofdb_sql::{Database, SqlTranslator};

fn database_path() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before Unix epoch")
        .as_nanos();
    env::temp_dir().join(format!("db-redb-automerge-example-{nanos}.redb"))
}

fn main() {
    block_on(async {
        let path = database_path();
        let database = Database::open(&path).expect("open Redb database");

        database
            .client()
            .translate_and_execute(
                "CREATE TABLE users (id UUID PRIMARY KEY, name TEXT)",
                &SqlTranslator,
            )
            .await
            .expect("create users");
        fs::remove_file(path).expect("remove Redb database");
    });
}
