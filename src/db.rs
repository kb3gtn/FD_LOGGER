use chrono::Utc;
use rusqlite::{params, Connection, Result};

use crate::models::{Contact, NewContact, SiteConfig};

pub fn init_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS site_config (
            id       INTEGER PRIMARY KEY,
            callsign TEXT NOT NULL,
            class    TEXT NOT NULL,
            section  TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS contacts (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            date       TEXT NOT NULL,
            time       TEXT NOT NULL,
            call       TEXT NOT NULL,
            band       TEXT NOT NULL,
            mode       TEXT NOT NULL,
            class      TEXT NOT NULL,
            section    TEXT NOT NULL,
            operator   TEXT NOT NULL,
            created_at TEXT NOT NULL,
            n1mm_id    TEXT
        );",
    )?;
    // Non-destructive migration for databases created before n1mm_id was added.
    let _ = conn.execute("ALTER TABLE contacts ADD COLUMN n1mm_id TEXT", []);
    Ok(())
}

pub fn get_site_config(conn: &Connection) -> Result<Option<SiteConfig>> {
    let mut stmt =
        conn.prepare("SELECT id, callsign, class, section FROM site_config LIMIT 1")?;
    let mut rows = stmt.query([])?;
    if let Some(row) = rows.next()? {
        Ok(Some(SiteConfig {
            id: row.get(0)?,
            callsign: row.get(1)?,
            class: row.get(2)?,
            section: row.get(3)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn save_site_config(conn: &Connection, config: &SiteConfig) -> Result<()> {
    conn.execute("DELETE FROM site_config", [])?;
    conn.execute(
        "INSERT INTO site_config (callsign, class, section) VALUES (?1, ?2, ?3)",
        params![config.callsign, config.class, config.section],
    )?;
    Ok(())
}

// n1mm_id: supply Some(&str) when the ID comes from an external source (N1MM),
//           or None to have the caller generate one beforehand and pass it in.
pub fn add_contact(
    conn: &Connection,
    contact: &NewContact,
    n1mm_id: Option<&str>,
) -> Result<Contact> {
    let now = Utc::now();
    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M").to_string();
    let created_at = now.to_rfc3339();

    let call     = contact.call.trim().to_uppercase();
    let class    = contact.class.trim().to_uppercase();
    let section  = contact.section.trim().to_uppercase();
    let operator = contact.operator.trim().to_uppercase();

    conn.execute(
        "INSERT INTO contacts
             (date, time, call, band, mode, class, section, operator, created_at, n1mm_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            date, time, call, contact.band, contact.mode,
            class, section, operator, created_at, n1mm_id
        ],
    )?;

    let id = conn.last_insert_rowid();
    Ok(Contact {
        id: Some(id),
        date,
        time,
        call,
        band: contact.band.clone(),
        mode: contact.mode.clone(),
        class,
        section,
        operator,
        n1mm_id: n1mm_id.map(|s| s.to_string()),
    })
}

/// Like add_contact but uses caller-supplied date/time instead of now().
/// Used by the N1MM listener to preserve the original QSO timestamp.
pub fn add_contact_with_time(
    conn: &Connection,
    contact: &NewContact,
    date: &str,
    time: &str,
    n1mm_id: Option<&str>,
) -> Result<Contact> {
    let created_at = Utc::now().to_rfc3339();
    let call     = contact.call.trim().to_uppercase();
    let class    = contact.class.trim().to_uppercase();
    let section  = contact.section.trim().to_uppercase();
    let operator = contact.operator.trim().to_uppercase();

    conn.execute(
        "INSERT INTO contacts
             (date, time, call, band, mode, class, section, operator, created_at, n1mm_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            date, time, call, contact.band, contact.mode,
            class, section, operator, created_at, n1mm_id
        ],
    )?;

    let id = conn.last_insert_rowid();
    Ok(Contact {
        id: Some(id),
        date: date.to_string(),
        time: time.to_string(),
        call,
        band: contact.band.clone(),
        mode: contact.mode.clone(),
        class,
        section,
        operator,
        n1mm_id: n1mm_id.map(|s| s.to_string()),
    })
}

fn row_to_contact(row: &rusqlite::Row) -> rusqlite::Result<Contact> {
    Ok(Contact {
        id:       row.get(0)?,
        date:     row.get(1)?,
        time:     row.get(2)?,
        call:     row.get(3)?,
        band:     row.get(4)?,
        mode:     row.get(5)?,
        class:    row.get(6)?,
        section:  row.get(7)?,
        operator: row.get(8)?,
        n1mm_id:  row.get(9)?,
    })
}

pub fn get_contacts(conn: &Connection, page: i64, per_page: i64) -> Result<(Vec<Contact>, i64)> {
    let offset = (page - 1) * per_page;
    let total: i64 =
        conn.query_row("SELECT COUNT(*) FROM contacts", [], |row| row.get(0))?;

    let mut stmt = conn.prepare(
        "SELECT id, date, time, call, band, mode, class, section, operator, n1mm_id
         FROM contacts ORDER BY id DESC LIMIT ?1 OFFSET ?2",
    )?;
    let contacts = stmt
        .query_map(params![per_page, offset], row_to_contact)?
        .collect::<Result<Vec<_>>>()?;

    Ok((contacts, total))
}

pub fn get_all_contacts_asc(conn: &Connection) -> Result<Vec<Contact>> {
    let mut stmt = conn.prepare(
        "SELECT id, date, time, call, band, mode, class, section, operator, n1mm_id
         FROM contacts ORDER BY id ASC",
    )?;
    let contacts = stmt
        .query_map([], row_to_contact)?
        .collect::<Result<Vec<_>>>()?;
    Ok(contacts)
}

pub fn get_contact(conn: &Connection, id: i64) -> Result<Option<Contact>> {
    let mut stmt = conn.prepare(
        "SELECT id, date, time, call, band, mode, class, section, operator, n1mm_id
         FROM contacts WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id])?;
    Ok(rows.next()?.map(|row| row_to_contact(row)).transpose()?)
}

pub fn update_contact(conn: &Connection, id: i64, contact: &NewContact) -> Result<bool> {
    let call     = contact.call.trim().to_uppercase();
    let class    = contact.class.trim().to_uppercase();
    let section  = contact.section.trim().to_uppercase();
    let operator = contact.operator.trim().to_uppercase();

    let rows = conn.execute(
        "UPDATE contacts
         SET call=?1, band=?2, mode=?3, class=?4, section=?5, operator=?6
         WHERE id=?7",
        params![call, contact.band, contact.mode, class, section, operator, id],
    )?;
    Ok(rows > 0)
}

pub fn delete_contact(conn: &Connection, id: i64) -> Result<bool> {
    let rows = conn.execute("DELETE FROM contacts WHERE id = ?1", params![id])?;
    Ok(rows > 0)
}

/// Returns all contacts with id > since_id, oldest first.
/// Used by the polling sync endpoint.
pub fn get_contacts_since(conn: &Connection, since_id: i64) -> Result<Vec<Contact>> {
    let mut stmt = conn.prepare(
        "SELECT id, date, time, call, band, mode, class, section, operator, n1mm_id
         FROM contacts WHERE id > ?1 ORDER BY id ASC",
    )?;
    let contacts = stmt
        .query_map(params![since_id], row_to_contact)?
        .collect::<Result<Vec<_>>>()?;
    Ok(contacts)
}

/// Returns true if (call, band, mode) is already in the log.
/// Field Day scoring treats this triple as the uniqueness key.
pub fn is_dupe(conn: &Connection, call: &str, band: &str, mode: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM contacts WHERE call=?1 AND band=?2 AND mode=?3",
        params![call, band, mode],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0)
        > 0
}

pub fn get_worked_sections(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT DISTINCT section FROM contacts")?;
    let sections = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>>>()?;
    Ok(sections)
}

// Used by the N1MM listener to avoid inserting duplicate contacts.
pub fn n1mm_id_exists(conn: &Connection, n1mm_id: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM contacts WHERE n1mm_id = ?1",
        params![n1mm_id],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0)
        > 0
}

pub fn update_by_n1mm_id(conn: &Connection, n1mm_id: &str, contact: &NewContact) -> Result<bool> {
    let call     = contact.call.trim().to_uppercase();
    let class    = contact.class.trim().to_uppercase();
    let section  = contact.section.trim().to_uppercase();
    let operator = contact.operator.trim().to_uppercase();

    let rows = conn.execute(
        "UPDATE contacts
         SET call=?1, band=?2, mode=?3, class=?4, section=?5, operator=?6
         WHERE n1mm_id=?7",
        params![call, contact.band, contact.mode, class, section, operator, n1mm_id],
    )?;
    Ok(rows > 0)
}

pub fn delete_by_n1mm_id(conn: &Connection, n1mm_id: &str) -> Result<bool> {
    let rows = conn.execute(
        "DELETE FROM contacts WHERE n1mm_id = ?1",
        params![n1mm_id],
    )?;
    Ok(rows > 0)
}
