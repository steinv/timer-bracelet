//! SQLite storage (replaces the Access DSdb.mdb of the original).
//! Times are stored as local "YYYY-MM-DD HH:MM:SS" text, which sorts correctly.

use std::path::Path;

use chrono::{Duration, NaiveDateTime};
use rusqlite::{params, Connection, OptionalExtension};

const TS: &str = "%Y-%m-%d %H:%M:%S";

/// SQL condition (with `?3` = now): the bracelet is no longer running.
const NOT_ACTIVE: &str = "NOT (ended_at IS NULL AND red_time >= ?3)";

pub struct Db {
    conn: Connection,
}

#[derive(Default, Clone, Copy)]
pub struct Online {
    pub green: i64,
    pub blue: i64,
    pub red: i64,
}

pub struct HistoryRow {
    pub start: NaiveDateTime,
    pub green: i64,
    pub blue: i64,
    pub red: i64,
    /// Set when the bracelet was turned off before its timer ran out.
    pub ended: Option<NaiveDateTime>,
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
        // v2.1: bracelets turned off early. Add the column to older databases.
        let has_ended: bool = conn
            .prepare("SELECT 1 FROM pragma_table_info('record') WHERE name = 'ended_at'")?
            .exists([])?;
        if !has_ended {
            conn.execute_batch("ALTER TABLE record ADD COLUMN ended_at TEXT")?;
        }
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

    /// A bracelet was turned off at `now`. The dongle doesn't say which one,
    /// so the oldest bracelet that is still running is marked as ended.
    /// Returns false if no bracelet was running.
    pub fn end_one_active(&self, now: NaiveDateTime) -> rusqlite::Result<bool> {
        let n = self.conn.execute(
            "UPDATE record SET ended_at = ?1 WHERE id = (
                 SELECT id FROM record
                 WHERE ended_at IS NULL AND start_time <= ?1 AND red_time >= ?1
                 ORDER BY start_time ASC LIMIT 1)",
            [ts(now)],
        )?;
        Ok(n > 0)
    }

    /// Number of timers currently in each phase (same rules as the original).
    pub fn online(&self, now: NaiveDateTime) -> rusqlite::Result<Online> {
        let n = ts(now);
        let q = |sql: &str| self.conn.query_row(sql, [&n], |r| r.get::<_, i64>(0));
        Ok(Online {
            green: q("SELECT COUNT(*) FROM record WHERE ended_at IS NULL AND green_time >= ?1")?,
            blue: q("SELECT COUNT(*) FROM record WHERE ended_at IS NULL AND green_time < ?1 AND yellow_time >= ?1")?,
            red: q("SELECT COUNT(*) FROM record WHERE ended_at IS NULL AND yellow_time < ?1 AND red_time >= ?1
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

    /// Newest first: (start time, green, blue, red minutes).
    pub fn list_between(
        &self,
        from: NaiveDateTime,
        to: NaiveDateTime,
        limit: usize,
    ) -> rusqlite::Result<Vec<HistoryRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT start_time,
                    CAST(ROUND((julianday(green_time)  - julianday(start_time))  * 1440) AS INTEGER),
                    CAST(ROUND((julianday(yellow_time) - julianday(green_time))  * 1440) AS INTEGER),
                    CAST(ROUND((julianday(red_time)    - julianday(yellow_time)) * 1440) AS INTEGER),
                    ended_at
             FROM record WHERE start_time BETWEEN ?1 AND ?2
             ORDER BY start_time DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![ts(from), ts(to), limit as i64], |r| {
            let start: String = r.get(0)?;
            Ok(HistoryRow {
                start: NaiveDateTime::parse_from_str(&start, TS).unwrap_or_default(),
                green: r.get(1)?,
                blue: r.get(2)?,
                red: r.get(3)?,
                ended: r
                    .get::<_, Option<String>>(4)?
                    .and_then(|s| NaiveDateTime::parse_from_str(&s, TS).ok()),
            })
        })?;
        rows.collect()
    }

    /// Records in the range that may be deleted at `now`: everything except
    /// bracelets that are still running (not turned off, red phase not over).
    /// Deleting a running one would make it vanish from the active counters.
    pub fn count_deletable(&self, from: NaiveDateTime, to: NaiveDateTime, now: NaiveDateTime) -> rusqlite::Result<i64> {
        self.conn.query_row(
            &format!("SELECT COUNT(*) FROM record WHERE start_time BETWEEN ?1 AND ?2 AND {NOT_ACTIVE}"),
            params![ts(from), ts(to), ts(now)],
            |r| r.get(0),
        )
    }

    /// Delete the records in the range, except bracelets still running at `now`.
    pub fn delete_between(&self, from: NaiveDateTime, to: NaiveDateTime, now: NaiveDateTime) -> rusqlite::Result<usize> {
        self.conn.execute(
            &format!("DELETE FROM record WHERE start_time BETWEEN ?1 AND ?2 AND {NOT_ACTIVE}"),
            params![ts(from), ts(to), ts(now)],
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
        let rows = db.list_between(at(9, 0), at(11, 0), 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].start, rows[0].green, rows[0].blue, rows[0].red), (at(10, 0), 10, 5, 5));
        assert_eq!(db.delete_between(at(9, 0), at(11, 0), at(12, 0)).unwrap(), 1);
        assert_eq!(db.count_between(at(0, 0), at(23, 59)).unwrap(), 0);
    }

    #[test]
    fn turning_off_removes_one_active_bracelet() {
        let db = Db::open(Path::new(":memory:")).unwrap();
        db.insert_record(at(10, 0), 10, 5, 5).unwrap();
        db.insert_record(at(10, 2), 10, 5, 5).unwrap();
        assert_eq!(db.online(at(10, 5)).unwrap().green, 2);

        assert!(db.end_one_active(at(10, 5)).unwrap());
        assert_eq!(db.online(at(10, 5)).unwrap().green, 1);
        // the oldest one was ended
        let rows = db.list_between(at(9, 0), at(11, 0), 10).unwrap();
        assert_eq!(rows[1].start, at(10, 0));
        assert_eq!(rows[1].ended, Some(at(10, 5)));
        assert_eq!(rows[0].ended, None);

        assert!(db.end_one_active(at(10, 6)).unwrap());
        assert!(!db.end_one_active(at(10, 7)).unwrap());
        assert_eq!(db.count_between(at(9, 0), at(11, 0)).unwrap(), 2);
    }

    #[test]
    fn delete_keeps_running_bracelets() {
        let db = Db::open(Path::new(":memory:")).unwrap();
        db.insert_record(at(9, 0), 10, 5, 5).unwrap(); // finished at 09:20
        db.insert_record(at(10, 0), 10, 5, 5).unwrap(); // turned off below
        db.insert_record(at(10, 30), 10, 5, 5).unwrap(); // still running at 10:35
        db.end_one_active(at(10, 10)).unwrap(); // ends the 10:00 one (only one running then)

        let now = at(10, 35);
        let (from, to) = (at(0, 0), at(23, 59));
        assert_eq!(db.count_between(from, to).unwrap(), 3);
        assert_eq!(db.count_deletable(from, to, now).unwrap(), 2);
        assert_eq!(db.delete_between(from, to, now).unwrap(), 2);

        // only the running bracelet is left, and it still counts as active
        let rows = db.list_between(from, to, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].start, at(10, 30));
        assert_eq!(db.online(now).unwrap().green, 1);
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
