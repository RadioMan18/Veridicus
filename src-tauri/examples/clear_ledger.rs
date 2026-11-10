use rusqlite::ffi::{sqlite3_key, SQLITE_OK};
use rusqlite::Connection;
use std::{env, fs, path::PathBuf};

fn open() -> Connection {
    let workspace = dirs::document_dir().expect("documents dir").join("Veridicus");
    let key_hex = fs::read_to_string(workspace.join(".veridicus").join(".db_key")).expect("key file");
    let key = hex::decode(key_hex.trim()).expect("hex key");
    let conn = Connection::open(workspace.join("veridicus.db")).expect("open db");
    unsafe {
        let rc = sqlite3_key(conn.handle(), key.as_ptr() as *const _, key.len() as _);
        assert_eq!(rc, SQLITE_OK, "sqlite3_key failed");
    }
    conn.busy_timeout(std::time::Duration::from_secs(5)).ok();
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
        .expect("wrong key or corrupt database");
    conn
}

fn main() {
    let conn = open();
    let workspace: PathBuf = dirs::document_dir().unwrap().join("Veridicus");
    let args: Vec<String> = env::args().skip(1).collect();

    let deleting_by_id = args.first().map(|s| s.as_str()) == Some("delete-id");
    if args.first().map(|s| s.as_str()) == Some("delete") || deleting_by_id {
        let selector = if deleting_by_id {
            args.get(1).expect("delete-id needs a uuid").clone()
        } else {
            args.get(1).expect("delete needs YYYY-MM-DD").clone()
        };
        let ids: Vec<String> = if deleting_by_id {
            vec![selector.clone()]
        } else {
            let mut stmt = conn
                .prepare("SELECT id FROM application_records WHERE date = ?1")
                .unwrap();
            let rows = stmt.query_map([&selector], |r| r.get::<_, String>(0)).unwrap();
            rows.filter_map(|r| r.ok()).collect()
        };
        println!("deleting {} record(s) for {}", ids.len(), selector);
        for id in &ids {
            conn.execute("DELETE FROM gaps WHERE application_id = ?1", [id]).ok();
            conn.execute("DELETE FROM application_records WHERE id = ?1", [id]).unwrap();
            println!("  removed {}", id);
        }

        // Drop only the matching rows from the CSV mirror, leaving other lines untouched.
        let csv_path = workspace.join("applications.csv");
        if let Ok(text) = fs::read_to_string(&csv_path) {
            let kept: Vec<&str> = text
                .lines()
                .filter(|line| {
                    !ids.iter().any(|id| line.starts_with(&format!("{},", id)))
                })
                .collect();
            fs::write(&csv_path, format!("{}\n", kept.join("\n"))).ok();
            println!("csv rows after rewrite: {}", kept.len());
        }
    }

    let mut stmt = conn
        .prepare("SELECT date, employer, role, status, id FROM application_records ORDER BY date DESC")
        .unwrap();
    let rows: Vec<(String, String, String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    println!("\n{} record(s):", rows.len());
    for (date, employer, role, status, id) in rows {
        println!("  {} | {} | {} | {} | {}", date, employer, role, status, id);
    }
}
