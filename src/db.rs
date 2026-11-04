//! SQLite storage (replaces the Access DSdb.mdb of the original).
//! Times are stored as local "YYYY-MM-DD HH:MM:SS" text, which sorts correctly.

use std::path::Path;

use chrono::{Duration, NaiveDateTime};
use rusqlite::{params, Connection, OptionalExtension};

const TS: &str = "%Y-%m-%d %H:%M:%S";

pub struct Db {
    conn: Connection,
}

#[derive(Default, Clone, Copy)]
pub struct Online {
    pub green: i64,
    pub blue: i64,
    pub red: i64,
}

fn ts(t: NaiveDateTime) -> String {
    t.format(TS).to_string()
}

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS record (
                 id          INTEGER PRIMARY KEY,
                 start_time  TEXT NOT NULL,
                 green_time  TEXT NOT NULL,
                 yellow_time TEXT NOT NULL,
                 red_time    TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS record_start ON record(start_time);
             CREATE TABLE IF NOT EXISTS setting (
                 key   TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );",
        )?;
        Ok(Self { conn })
    }

    /// A timer was programmed at `now`: it is green for `g` minutes, then
    /// blue/yellow for `y`, then red for `r`.
    pub fn insert_record(&self, now: NaiveDateTime, g: u16, y: u16, r: u16) -> rusqlite::Result<()> {
        let green = now + Duration::minutes(g as i64);
        let yellow = green + Duration::minutes(y as i64);
        let red = yellow + Duration::minutes(r as i64);
        self.conn.execute(
            "INSERT INTO record (start_time, green_time, yellow_time, red_time) VALUES (?1, ?2, ?3, ?4)",
            params![ts(now), ts(green), ts(yellow), ts(red)],
        )?;
        Ok(())
    }

    /// Number of timers currently in each phase (same rules as the original).
    pub fn online(&self, now: NaiveDateTime) -> rusqlite::Result<Online> {
        let n = ts(now);
        let q = |sql: &str| self.conn.query_row(sql, [&n], |r| r.get::<_, i64>(0));
        Ok(Online {
            green: q("SELECT COUNT(*) FROM record WHERE green_time >= ?1")?,
            blue: q("SELECT COUNT(*) FROM record WHERE green_time < ?1 AND yellow_time >= ?1")?,
            red: q("SELECT COUNT(*) FROM record WHERE yellow_time < ?1 AND red_time >= ?1
                    AND date(yellow_time) = date(?1) AND date(red_time) = date(?1)")?,
        })
    }

    pub fn count_between(&self, from: NaiveDateTime, to: NaiveDateTime) -> rusqlite::Result<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM record WHERE start_time BETWEEN ?1 AND ?2",
            params![ts(from), ts(to)],
            |r| r.get(0),
        )
    }

    pub fn delete_between(&self, from: NaiveDateTime, to: NaiveDateTime) -> rusqlite::Result<usize> {
        self.conn.execute(
            "DELETE FROM record WHERE start_time BETWEEN ?1 AND ?2",
            params![ts(from), ts(to)],
        )
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.conn
            .query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    }

    pub fn set(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO setting (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 4).unwrap().and_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn phases_and_ranges() {
        let db = Db::open(Path::new(":memory:")).unwrap();
        db.insert_record(at(10, 0), 10, 5, 5).unwrap(); // green until 10:10, blue until 10:15, red until 10:20

        let o = db.online(at(10, 5)).unwrap();
        assert_eq!((o.green, o.blue, o.red), (1, 0, 0));
        let o = db.online(at(10, 12)).unwrap();
        assert_eq!((o.green, o.blue, o.red), (0, 1, 0));
        let o = db.online(at(10, 18)).unwrap();
        assert_eq!((o.green, o.blue, o.red), (0, 0, 1));
        let o = db.online(at(10, 30)).unwrap();
        assert_eq!((o.green, o.blue, o.red), (0, 0, 0));

        assert_eq!(db.count_between(at(9, 0), at(11, 0)).unwrap(), 1);
        assert_eq!(db.count_between(at(11, 0), at(12, 0)).unwrap(), 0);
        assert_eq!(db.delete_between(at(9, 0), at(11, 0)).unwrap(), 1);
        assert_eq!(db.count_between(at(0, 0), at(23, 59)).unwrap(), 0);
    }

    #[test]
    fn settings() {
        let db = Db::open(Path::new(":memory:")).unwrap();
        assert_eq!(db.get("green"), None);
        db.set("green", "5").unwrap();
        db.set("green", "7").unwrap();
        assert_eq!(db.get("green").as_deref(), Some("7"));
    }
}
