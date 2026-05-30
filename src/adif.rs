//! ADIF file import — parses .adi / .adif files into log entries.
//!
//! Field Day relevant ADIF fields extracted:
//!   CALL, BAND, MODE, QSO_DATE, TIME_ON, CLASS, ARRL_SECT, SRX_STRING, OPERATOR

use chrono::Utc;
use crate::sections::VALID_ABBREVS;

pub struct AdifEntry {
    pub call:     String,
    pub band:     String,
    pub mode:     String,
    pub date:     String,   // YYYY-MM-DD
    pub time:     String,   // HH:MM
    pub class:    String,
    pub section:  String,
    pub operator: String,
}

// ── Field extractor ───────────────────────────────────────────────────────────

/// Pull the value of `<FIELD:LEN>value` from a single record string.
fn field(record: &str, name: &str) -> String {
    let upper   = record.to_uppercase();
    let tag     = format!("<{}:", name.to_uppercase());
    let pos     = match upper.find(&tag) { Some(p) => p, None => return String::new() };
    let rest_u  = &upper[pos + tag.len()..];
    let len_end = match rest_u.find('>') { Some(p) => p, None => return String::new() };
    let len: usize = match rest_u[..len_end].trim().parse() { Ok(n) => n, Err(_) => return String::new() };
    let v_start = pos + tag.len() + len_end + 1;
    if v_start > record.len() { return String::new(); }
    record[v_start..(v_start + len).min(record.len())].trim().to_string()
}

// ── Conversions ───────────────────────────────────────────────────────────────

fn band_to_ours(b: &str) -> Option<&'static str> {
    match b.to_lowercase().trim() {
        "160m" => Some("160M"),
        "80m"  => Some("80M"),
        "40m"  => Some("40M"),
        "20m"  => Some("20M"),
        "15m"  => Some("15M"),
        "10m"  => Some("10M"),
        "6m"   => Some("6M"),
        "2m"   => Some("2M"),
        "70cm" => Some("70CM"),
        _      => None,
    }
}

fn mode_to_ours(m: &str) -> &'static str {
    match m.to_uppercase().trim() {
        "CW"                                  => "CW",
        "SSB"|"USB"|"LSB"|"AM"|"FM"|"PHONE"  => "PH",
        _                                     => "DIG",
    }
}

// ── Parser ────────────────────────────────────────────────────────────────────

/// Parse a complete ADIF file and return valid log entries.
pub fn parse(content: &str) -> Vec<AdifEntry> {
    // Body starts after the header marker <EOH>
    let upper      = content.to_uppercase();
    let body_start = upper.find("<EOH>").map(|i| i + 5).unwrap_or(0);
    let body       = &content[body_start..];
    let body_upper = body.to_uppercase();

    let mut entries = Vec::new();
    let mut pos = 0;

    while let Some(eor_rel) = body_upper[pos..].find("<EOR>") {
        let record = &body[pos..pos + eor_rel];
        pos += eor_rel + 5;
        if record.trim().is_empty() { continue; }

        // ── Required: call sign ──────────────────────────────────────────────
        let call = field(record, "call").trim().to_uppercase();
        if call.is_empty() { continue; }

        // ── Required: band (skip if unknown) ────────────────────────────────
        let band = match band_to_ours(&field(record, "band")) {
            Some(b) => b.to_string(),
            None    => continue,
        };

        // ── Mode ─────────────────────────────────────────────────────────────
        let mode = mode_to_ours(&field(record, "mode")).to_string();

        // ── Date: YYYYMMDD → YYYY-MM-DD ─────────────────────────────────────
        let date_raw = field(record, "qso_date");
        let date = if date_raw.len() == 8 {
            format!("{}-{}-{}", &date_raw[..4], &date_raw[4..6], &date_raw[6..8])
        } else {
            Utc::now().format("%Y-%m-%d").to_string()
        };

        // ── Time: HHMMSS or HHMM → HH:MM ────────────────────────────────────
        let time_raw = field(record, "time_on");
        let time = if time_raw.len() >= 4 {
            format!("{}:{}", &time_raw[..2], &time_raw[2..4])
        } else {
            Utc::now().format("%H:%M").to_string()
        };

        // ── Class: CLASS field, else first token of SRX_STRING ───────────────
        let class = {
            let c = field(record, "class").to_uppercase();
            if !c.is_empty() { c } else {
                field(record, "srx_string")
                    .split_whitespace().next().unwrap_or("").to_uppercase()
            }
        };

        // ── Section: ARRL_SECT field, else last token of SRX_STRING ─────────
        let section = {
            let raw = field(record, "arrl_sect").trim().to_uppercase();
            let candidate = if !raw.is_empty() { raw } else {
                let srx   = field(record, "srx_string");
                let parts: Vec<&str> = srx.split_whitespace().collect();
                parts.last().map(|s| s.to_uppercase()).unwrap_or_default()
            };
            if VALID_ABBREVS.contains(&candidate.as_str()) { candidate } else { "DX".to_string() }
        };

        // ── Operator ─────────────────────────────────────────────────────────
        let operator = field(record, "operator").trim().to_uppercase();

        entries.push(AdifEntry { call, band, mode, date, time, class, section, operator });
    }

    entries
}
