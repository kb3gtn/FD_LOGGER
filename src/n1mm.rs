//! N1MM+ UDP network message support.
//!
//! N1MM+ broadcasts XML messages on UDP port 12060 whenever a contact is
//! logged, replaced, or deleted.  We listen for those messages so N1MM
//! operators are automatically synced into this log, and we broadcast our
//! own messages so N1MM sees contacts entered here.
//!
//! Port:  12060 (listen on 0.0.0.0; broadcast to configurable address)
//! N1MM band field: MHz float strings ("3.5", "14", "50", …)
//! N1MM mode field: "CW", "SSB", "USB", "LSB", "DIG", "RTTY", "FT8", …

use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};

use socket2::{Domain, Protocol, Socket, Type};

use chrono::Utc;
use rusqlite::Connection;

use crate::db;
use crate::models::{NewContact, SiteConfig, Contact};
use crate::sections::VALID_ABBREVS;

pub const PORT: u16 = 12060;
static ID_SEQ: AtomicU64 = AtomicU64::new(0);

// ── Unique ID ────────────────────────────────────────────────────────────────

/// Generate a 32-char hex string compatible with N1MM's ID field.
pub fn new_id() -> String {
    let secs = Utc::now().timestamp() as u64;
    let seq  = ID_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{:016x}{:016x}", secs, seq)
}

// ── Band conversion ──────────────────────────────────────────────────────────

/// Our band label → N1MM MHz string.
pub fn band_to_mhz(band: &str) -> &'static str {
    match band {
        "160M" => "1.8",
        "80M"  => "3.5",
        "40M"  => "7",
        "20M"  => "14",
        "15M"  => "21",
        "10M"  => "28",
        "6M"   => "50",
        "2M"   => "144",
        "70CM" => "432",
        _      => "14",
    }
}

/// N1MM MHz string → our band label. Uses frequency ranges so fractional
/// values ("3.573") are handled correctly.
pub fn mhz_to_band(mhz: &str) -> &'static str {
    let f: f64 = mhz.parse().unwrap_or(14.0);
    match f as u32 {
        0..=1          => "160M",
        2..=4          => "80M",
        5..=9          => "40M",
        10..=17        => "20M",
        18..=24        => "15M",
        25..=49        => "10M",
        50..=143       => "6M",
        144..=431      => "2M",
        _              => "70CM",
    }
}

// ── Mode conversion ──────────────────────────────────────────────────────────

/// Our mode → N1MM radio mode string.
/// N1MM uses the actual submode (LSB/USB/CW/DIG), not the generic "SSB".
/// Phone below 10m is conventionally LSB; 10m and above is USB.
pub fn mode_to_n1mm(mode: &str, band: &str) -> &'static str {
    match mode {
        "CW"  => "CW",
        "DIG" => "DIG",
        "PH"  => match band {
            "10M" | "6M" | "2M" | "70CM" => "USB",
            _                              => "LSB",
        },
        _ => "LSB",
    }
}

/// N1MM mode string → our mode.
pub fn n1mm_to_mode(mode: &str) -> &'static str {
    match mode.to_uppercase().as_str() {
        "CW"                          => "CW",
        "SSB" | "USB" | "LSB" | "AM" | "FM" => "PH",
        _                             => "DIG",
    }
}

// ── XML helpers ──────────────────────────────────────────────────────────────

/// Extract the text content of the first occurrence of <tag>…</tag>.
fn xml_field(xml: &str, tag: &str) -> String {
    let open  = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    xml.find(&open)
        .map(|s| s + open.len())
        .and_then(|start| {
            xml[start..].find(&close).map(|end| xml[start..start + end].trim().to_string())
        })
        .unwrap_or_default()
}

/// Return the name of the root XML element (e.g. "contactinfo").
fn root_tag(xml: &str) -> &str {
    let xml = xml.trim_start_matches(|c: char| c != '<');
    // Skip the XML declaration if present
    let xml = if xml.starts_with("<?") {
        xml.find("?>").map(|i| &xml[i + 2..]).unwrap_or(xml).trim()
    } else {
        xml
    };
    let start = xml.find('<').map(|i| i + 1).unwrap_or(0);
    let rest  = &xml[start..];
    let end   = rest.find(|c: char| c == '>' || c == ' ' || c == '/').unwrap_or(rest.len());
    &rest[..end]
}


fn is_valid_section(s: &str) -> bool {
    VALID_ABBREVS.contains(&s)
}

// ── XML builders ─────────────────────────────────────────────────────────────

pub fn build_contactinfo(c: &Contact, cfg: &SiteConfig, id: &str) -> String {
    let ts   = format!("{} {}:00", c.date, c.time);
    let band = band_to_mhz(&c.band);
    let mode = mode_to_n1mm(&c.mode, &c.band);
    let sent = format!("{} {}", cfg.class, cfg.section);
    xml_envelope("contactinfo", c, cfg, id, &ts, band, mode, &sent)
}

pub fn build_contactreplace(c: &Contact, cfg: &SiteConfig, id: &str) -> String {
    let ts   = format!("{} {}:00", c.date, c.time);
    let band = band_to_mhz(&c.band);
    let mode = mode_to_n1mm(&c.mode, &c.band);
    let sent = format!("{} {}", cfg.class, cfg.section);
    xml_envelope("contactreplace", c, cfg, id, &ts, band, mode, &sent)
}

pub fn build_contactdelete(c: &Contact, cfg: &SiteConfig) -> String {
    let id = c.n1mm_id.as_deref().unwrap_or("");
    let ts = format!("{} {}:00", c.date, c.time);
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <contactdelete>\n\
         <app>N1MM</app>\n\
         <timestamp>{ts}</timestamp>\n\
         <mycall>{mycall}</mycall>\n\
         <band>{band}</band>\n\
         <call>{call}</call>\n\
         <contestnr>1</contestnr>\n\
         <StationName>FDLogger</StationName>\n\
         <ID>{id}</ID>\n\
         </contactdelete>",
        ts     = ts,
        mycall = cfg.callsign,
        band   = band_to_mhz(&c.band),
        call   = c.call,
        id     = id,
    )
}

/// N1MM rxfreq/txfreq: kHz × 100 for each band's typical phone calling freq.
fn band_to_freq100(band_mhz: &str) -> &'static str {
    match band_mhz {
        "1.8"  => "182500",   // 1825 kHz
        "3.5"  => "390000",   // 3900 kHz (80m phone)
        "7"    => "712500",   // 7125 kHz (40m phone)
        "14"   => "1422500",  // 14225 kHz
        "21"   => "2130000",  // 21300 kHz
        "28"   => "2840000",  // 28400 kHz
        "50"   => "5012500",  // 50125 kHz
        "144"  => "14420000", // 144200 kHz
        "432"  => "43210000", // 432100 kHz
        _      => "1422500",
    }
}

/// Build a Cabrillo QSO line to include in the XML (informational for N1MM).
fn cabrillo_string(c: &Contact, cfg: &SiteConfig, band_mhz: &str) -> String {
    // Frequency in kHz (no decimal)
    let freq_khz = band_to_freq100(band_mhz)
        .parse::<u64>()
        .unwrap_or(1422500) / 100;
    let mode_cab = match c.mode.as_str() {
        "CW"  => "CW",
        "DIG" => "DIG",
        _     => "PH",
    };
    // Date YYYY-MM-DD, time HHMM
    let time = c.time.replace(':', "");
    let sent = format!("{} {}", cfg.class, cfg.section);
    let rcvd = format!("{} {}", c.class, c.section);
    format!(
        "QSO: {:>6} {} {} {} {:<13} {:<6} {:<3} {:<13} {:<6} {:<3}",
        freq_khz, mode_cab, c.date, &time[..4.min(time.len())],
        cfg.callsign, cfg.class, cfg.section,
        c.call, c.class, c.section,
    ).replace("  ", " ") // collapse extra spaces in sent/rcvd if short
    // Note: exact column alignment not critical; N1MM uses this for display only
    + &format!("  sent={} rcvd={}", sent, rcvd) // fallback readable form
}

fn xml_envelope(
    root: &str,
    c: &Contact,
    cfg: &SiteConfig,
    id: &str,
    ts: &str,
    band_mhz: &str,
    mode: &str,
    sent: &str,
) -> String {
    let freq = band_to_freq100(band_mhz);
    let cabrillo = cabrillo_string(c, cfg, band_mhz);
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <{root}>\n\
         \t<app>N1MM</app>\n\
         \t<contestname>FD</contestname>\n\
         \t<dbname>fd_logger.db</dbname>\n\
         \t<contestnr>1</contestnr>\n\
         \t<timestamp>{ts}</timestamp>\n\
         \t<mycall>{mycall}</mycall>\n\
         \t<band>{band}</band>\n\
         \t<rxfreq>{freq}</rxfreq>\n\
         \t<txfreq>{freq}</txfreq>\n\
         \t<operator>{operator}</operator>\n\
         \t<mode>{mode}</mode>\n\
         \t<call>{call}</call>\n\
         \t<countryprefix>K</countryprefix>\n\
         \t<wpxprefix></wpxprefix>\n\
         \t<stationprefix>{mycall}</stationprefix>\n\
         \t<continent>NA</continent>\n\
         \t<snt>59</snt>\n\
         \t<sntnr>0</sntnr>\n\
         \t<rcv>59</rcv>\n\
         \t<rcvnr>0</rcvnr>\n\
         \t<gridsquare></gridsquare>\n\
         \t<exchange1>{class}</exchange1>\n\
         \t<section>{section}</section>\n\
         \t<comment></comment>\n\
         \t<qth></qth>\n\
         \t<name></name>\n\
         \t<power></power>\n\
         \t<misctext></misctext>\n\
         \t<zone>0</zone>\n\
         \t<prec></prec>\n\
         \t<ck>0</ck>\n\
         \t<ismultiplier1>0</ismultiplier1>\n\
         \t<ismultiplier2>0</ismultiplier2>\n\
         \t<ismultiplier3>0</ismultiplier3>\n\
         \t<points>1</points>\n\
         \t<radionr>1</radionr>\n\
         \t<run1run2>1</run1run2>\n\
         \t<RoverLocation></RoverLocation>\n\
         \t<RadioInterfaced>0</RadioInterfaced>\n\
         \t<NetworkedCompNr>0</NetworkedCompNr>\n\
         \t<IsOriginal>True</IsOriginal>\n\
         \t<NetBiosName>FDLogger</NetBiosName>\n\
         \t<IsRunQSO>0</IsRunQSO>\n\
         \t<StationName>FDLogger</StationName>\n\
         \t<ID>{id}</ID>\n\
         \t<IsClaimedQso>1</IsClaimedQso>\n\
         \t<oldtimestamp>{ts}</oldtimestamp>\n\
         \t<oldcall>{call}</oldcall>\n\
         \t<SentExchange>{sent}</SentExchange>\n\
         \t<CabrilloString>{cabrillo}</CabrilloString>\n\
         </{root}>",
        root     = root,
        ts       = ts,
        mycall   = cfg.callsign,
        band     = band_mhz,
        freq     = freq,
        operator = c.operator,
        mode     = mode,
        call     = c.call,
        class    = c.class,
        section  = c.section,
        sent     = sent,
        id       = id,
        cabrillo = cabrillo,
    )
}

// ── Broadcast (synchronous, fire-and-forget) ─────────────────────────────────

/// Send one UDP datagram.  Uses std (blocking) socket because sends complete
/// almost instantly and callers are already in a synchronous context.
pub fn broadcast(xml: &str, addr: &str, port: u16) {
    let target = format!("{}:{}", addr, port);
    println!("[N1MM] ▶ broadcasting to {}\n{}\n[N1MM] ---", target, xml.trim());
    match UdpSocket::bind("0.0.0.0:0") {
        Ok(sock) => {
            let _ = sock.set_broadcast(true);
            match sock.send_to(xml.as_bytes(), &target) {
                Ok(n)  => println!("[N1MM] ▶ sent {} bytes to {}", n, target),
                Err(e) => eprintln!("[N1MM] send error: {}", e),
            }
        }
        Err(e) => eprintln!("[N1MM] socket error: {}", e),
    }
}

// ── Listener (async, runs as a background Tokio task) ────────────────────────

pub async fn run_listener(db_path: String, port: u16) {
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
    let raw = match Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)) {
        Ok(s)  => s,
        Err(e) => { eprintln!("[N1MM] Cannot create UDP socket: {}", e); return; }
    };
    let _ = raw.set_reuse_address(true);
    if let Err(e) = raw.bind(&addr.into()) {
        eprintln!("[N1MM] Cannot bind UDP :{} – {}", port, e);
        return;
    }
    raw.set_nonblocking(true).unwrap();
    let std_sock: UdpSocket = raw.into();
    let sock = match rocket::tokio::net::UdpSocket::from_std(std_sock) {
        Ok(s)  => { println!("[N1MM] Listening on UDP :{}", port); s }
        Err(e) => { eprintln!("[N1MM] Cannot convert to tokio socket: {}", e); return; }
    };
    let _ = sock.set_broadcast(true);
    let _ = sock.join_multicast_v4(
        std::net::Ipv4Addr::new(239, 255, 0, 0), 
        std::net::Ipv4Addr::UNSPECIFIED,
    );

    let mut buf = vec![0u8; 65_535];
    loop {
        let (len, from) = match sock.recv_from(&mut buf).await {
            Ok(r)  => r,
            Err(e) => { eprintln!("[N1MM] recv error: {}", e); continue; }
        };
        let xml = match std::str::from_utf8(&buf[..len]) {
            Ok(s)  => s.to_string(),
            Err(_) => {
                eprintln!("[N1MM] ← {} bytes from {} (non-UTF8, ignored)", len, from);
                continue;
            }
        };

        println!("[N1MM] ← {} bytes from {}", len, from);
        println!("[N1MM] raw:\n{}\n[N1MM] ---", xml.trim());

        // Ignore our own broadcasts (we now send app=N1MM, so key off StationName)
        let station = xml_field(&xml, "StationName");
        if station == "FDLogger" {
            println!("[N1MM] ← ignoring our own broadcast");
            continue;
        }

        let tag = root_tag(&xml).to_string();
        println!("[N1MM] ← root tag: {:?}  station: {:?}", tag, station);
        let db_path = db_path.clone();
        rocket::tokio::task::spawn_blocking(move || {
            match tag.as_str() {
                "contactinfo"    => handle_contactinfo(&xml, &db_path),
                "contactreplace" => handle_contactreplace(&xml, &db_path),
                "contactdelete"  => handle_contactdelete(&xml, &db_path),
                other            => {
                    if !["AppInfo","RadioInfo","spot","Spectrum"].contains(&other) {
                        println!("[N1MM] ← ignored message type: {}", other);
                    }
                }
            }
        });

        let _ = from; // suppress unused warning
    }
}

// ── Incoming message handlers ────────────────────────────────────────────────

fn handle_contactinfo(xml: &str, db_path: &str) {
    let n1mm_id   = xml_field(xml, "ID");
    let call      = xml_field(xml, "call").to_uppercase();
    let band_mhz  = xml_field(xml, "band");
    let mode_raw  = xml_field(xml, "mode");
    let section   = xml_field(xml, "section").trim().to_uppercase();
    let class     = xml_field(xml, "exchange1").trim().to_uppercase();
    let operator  = xml_field(xml, "operator").to_uppercase();
    let timestamp = xml_field(xml, "timestamp");

    if call.is_empty() {
        return;
    }

    let band = mhz_to_band(&band_mhz).to_string();
    let mode = n1mm_to_mode(&mode_raw).to_string();

    // Validate section — fall back to DX for foreign stations
    let sect = if is_valid_section(&section) { section } else { "DX".to_string() };

    let (date, time) = split_timestamp(&timestamp);

    println!("[N1MM] contactinfo parsed: call={} band={} ({}MHz) mode={} ({}) class={} section={} op={} ts={}",
        call, band, band_mhz, mode, mode_raw, class, sect, operator, timestamp);

    let contact = NewContact { call: call.clone(), band, mode, class, section: sect, operator };

    match Connection::open(db_path) {
        Ok(conn) => {
            // Reject if the same GUID is already stored
            if !n1mm_id.is_empty() && db::n1mm_id_exists(&conn, &n1mm_id) {
                println!("[N1MM] ← duplicate GUID for {} ({}), skipped", call, n1mm_id);
                return;
            }
            // Reject if (call, band, mode) already exists — field day dupe
            if db::is_dupe(&conn, &contact.call, &contact.band, &contact.mode) {
                println!("[N1MM] ← dupe QSO {} {} {}, skipped", contact.call, contact.band, contact.mode);
                return;
            }
            // Override timestamp to the one from N1MM rather than now
            match db::add_contact_with_time(&conn, &contact, &date, &time, Some(&n1mm_id)) {
                Ok(_)  => println!("[N1MM] ← logged {} on {} {}", call, contact.band, contact.mode),
                Err(e) => eprintln!("[N1MM] DB insert error: {}", e),
            }
        }
        Err(e) => eprintln!("[N1MM] DB open error: {}", e),
    }
}

fn handle_contactreplace(xml: &str, db_path: &str) {
    let n1mm_id  = xml_field(xml, "ID");
    let call     = xml_field(xml, "call").to_uppercase();
    let band_mhz = xml_field(xml, "band");
    let mode_raw = xml_field(xml, "mode");
    let section  = xml_field(xml, "section").trim().to_uppercase();
    let class    = xml_field(xml, "exchange1").trim().to_uppercase();
    let operator = xml_field(xml, "operator").to_uppercase();

    if call.is_empty() || n1mm_id.is_empty() {
        return;
    }

    let band = mhz_to_band(&band_mhz).to_string();
    let mode = n1mm_to_mode(&mode_raw).to_string();
    let sect = if is_valid_section(&section) { section } else { "DX".to_string() };

    let contact = NewContact { call: call.clone(), band, mode, class, section: sect, operator };

    match Connection::open(db_path) {
        Ok(conn) => match db::update_by_n1mm_id(&conn, &n1mm_id, &contact) {
            Ok(true)  => println!("[N1MM] ← updated {} ({})", call, n1mm_id),
            Ok(false) => {
                // N1MM sometimes sends a replace without a prior delete; treat as new.
                let _ = db::add_contact(&conn, &contact, Some(&n1mm_id));
                println!("[N1MM] ← replace-as-insert {} ({})", call, n1mm_id);
            }
            Err(e) => eprintln!("[N1MM] DB update error: {}", e),
        },
        Err(e) => eprintln!("[N1MM] DB open error: {}", e),
    }
}

fn handle_contactdelete(xml: &str, db_path: &str) {
    let n1mm_id = xml_field(xml, "ID");
    let call    = xml_field(xml, "call");

    if n1mm_id.is_empty() {
        return;
    }

    match Connection::open(db_path) {
        Ok(conn) => match db::delete_by_n1mm_id(&conn, &n1mm_id) {
            Ok(true)  => println!("[N1MM] ← deleted {} ({})", call, n1mm_id),
            Ok(false) => println!("[N1MM] ← delete for unknown ID {}", n1mm_id),
            Err(e)    => eprintln!("[N1MM] DB delete error: {}", e),
        },
        Err(e) => eprintln!("[N1MM] DB open error: {}", e),
    }
}

// ── Timestamp helpers ────────────────────────────────────────────────────────

/// Split "2020-01-17 16:43:38" into ("2020-01-17", "16:43").
fn split_timestamp(ts: &str) -> (String, String) {
    let mut parts = ts.splitn(2, ' ');
    let date = parts.next().unwrap_or("").to_string();
    let time_full = parts.next().unwrap_or("").to_string();
    let time = if time_full.len() >= 5 { time_full[..5].to_string() } else { time_full };
    (date, time)
}
