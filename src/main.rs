#[macro_use]
extern crate rocket;

use rocket::fairing::{Fairing, Info, Kind};
use rocket::data::{Data, ToByteUnit};
use rocket::form::Form;
use rocket::http::{ContentType, Header};
use rocket::response::{Redirect, Response, Responder};
use rocket::serde::json::Json;
use rocket::{Orbit, Request, Rocket, State};
use rocket_dyn_templates::{context, Template};
use rusqlite::Connection;
use std::io::Cursor;
use std::sync::Mutex;

mod adif;
mod db;
mod models;
mod n1mm;
mod sections;

use models::{ApiResponse, Contact, NewContact, SiteConfig};

// ── Application state ────────────────────────────────────────────────────────

pub struct DbState(Mutex<Connection>);

// ── ADIF download responder ───────────────────────────────────────────────────

pub struct AdifDownload(Vec<u8>);

impl<'r, 'o: 'r> Responder<'r, 'o> for AdifDownload {
    fn respond_to(self, _req: &'r Request<'_>) -> rocket::response::Result<'o> {
        Response::build()
            .header(ContentType::new("application", "octet-stream"))
            .header(Header::new(
                "Content-Disposition",
                "attachment; filename=\"field_day_log.adi\"",
            ))
            .sized_body(self.0.len(), Cursor::new(self.0))
            .ok()
    }
}

// ── N1MM background listener fairing ────────────────────────────────────────

struct N1mmFairing {
    db_path: String,
}

#[rocket::async_trait]
impl Fairing for N1mmFairing {
    fn info(&self) -> Info {
        Info { name: "N1MM UDP Listener", kind: Kind::Liftoff }
    }
    async fn on_liftoff(&self, _rocket: &Rocket<Orbit>) {
        let db_path = self.db_path.clone();
        rocket::tokio::spawn(n1mm::run_listener(db_path, n1mm::PORT));
    }
}

// ── Input validation ─────────────────────────────────────────────────────────

const VALID_BANDS: &[&str] = &["160M", "80M", "40M", "20M", "15M", "10M", "6M", "2M", "70CM"];
const VALID_MODES: &[&str] = &["PH", "CW", "DIG"];
const PER_PAGE: i64 = 25;

fn validate_contact(c: &NewContact) -> Result<(), String> {
    let call = c.call.trim();
    if call.len() < 3 {
        return Err("Call sign too short (min 3 characters)".into());
    }
    if !call.chars().all(|ch| ch.is_alphanumeric() || ch == '/' || ch == '-') {
        return Err("Call sign contains invalid characters".into());
    }
    if !VALID_BANDS.contains(&c.band.as_str()) {
        return Err(format!("Invalid band: {}", c.band));
    }
    if !VALID_MODES.contains(&c.mode.as_str()) {
        return Err(format!("Invalid mode: {}", c.mode));
    }
    let cls = c.class.trim().to_uppercase();
    let letter_pos = cls.chars().position(|ch| ch.is_alphabetic());
    match letter_pos {
        None => return Err("Class must be a number followed by A-F (e.g., 4A)".into()),
        Some(pos) => {
            if cls[..pos].parse::<u32>().is_err() {
                return Err("Class prefix must be a number".into());
            }
            if !["A", "B", "C", "D", "E", "F"].contains(&&cls[pos..]) {
                return Err("Class letter must be A-F".into());
            }
        }
    }
    let sect = c.section.trim().to_uppercase();
    if !sections::VALID_ABBREVS.contains(&sect.as_str()) {
        return Err(format!("\"{}\" is not a valid ARRL/RAC section or DX", sect));
    }
    Ok(())
}

// ── Page routes ──────────────────────────────────────────────────────────────

#[get("/")]
fn index(db: &State<DbState>) -> Redirect {
    let conn = db.0.lock().unwrap();
    match db::get_site_config(&conn) {
        Ok(Some(_)) => Redirect::to("/logger"),
        _ => Redirect::to("/setup"),
    }
}

#[derive(FromForm)]
struct SetupForm {
    callsign: String,
    class: String,
    section: String,
}

#[get("/setup")]
fn setup_get(db: &State<DbState>) -> Template {
    let conn = db.0.lock().unwrap();
    let existing = db::get_site_config(&conn).ok().flatten();
    Template::render("setup", context! { existing })
}

#[post("/setup", data = "<form>")]
fn setup_post(db: &State<DbState>, form: Form<SetupForm>) -> Redirect {
    let conn = db.0.lock().unwrap();
    let config = SiteConfig {
        id: None,
        callsign: form.callsign.trim().to_uppercase(),
        class: form.class.trim().to_uppercase(),
        section: form.section.trim().to_uppercase(),
    };
    let _ = db::save_site_config(&conn, &config);
    Redirect::to("/logger")
}

#[get("/logger?<page>")]
fn logger(db: &State<DbState>, page: Option<i64>) -> Result<Template, Redirect> {
    let conn = db.0.lock().unwrap();
    let config = match db::get_site_config(&conn) {
        Ok(Some(c)) => c,
        _ => return Err(Redirect::to("/setup")),
    };

    let page = page.unwrap_or(1).max(1);
    let (contacts, total) = db::get_contacts(&conn, page, PER_PAGE).unwrap_or_default();
    let worked_sections = db::get_worked_sections(&conn).unwrap_or_default();
    let districts = sections::all_districts();

    let total_pages = ((total + PER_PAGE - 1) / PER_PAGE).max(1);
    let current_page = page.min(total_pages);
    let total_sections: i64 = districts.iter().map(|d| d.sections.len() as i64).sum();
    let worked_count = worked_sections.len() as i64;

    Ok(Template::render(
        "logger",
        context! {
            config,
            contacts,
            total,
            current_page,
            total_pages,
            worked_sections,
            worked_count,
            total_sections,
            districts,
        },
    ))
}

// ── API routes ───────────────────────────────────────────────────────────────

#[post("/api/contacts", data = "<body>")]
fn api_add_contact(
    db: &State<DbState>,
    body: Json<NewContact>,
) -> Json<ApiResponse<Contact>> {
    if let Err(e) = validate_contact(&body) {
        return Json(ApiResponse::err(e));
    }
    let id = n1mm::new_id();
    let conn = db.0.lock().unwrap();
    match db::add_contact(&conn, &body, Some(&id)) {
        Ok(c)  => Json(ApiResponse::ok(c)),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

#[put("/api/contacts/<id>", data = "<body>")]
fn api_update_contact(
    db: &State<DbState>,
    id: i64,
    body: Json<NewContact>,
) -> Json<ApiResponse<Contact>> {
    if let Err(e) = validate_contact(&body) {
        return Json(ApiResponse::err(e));
    }
    let conn = db.0.lock().unwrap();
    match db::update_contact(&conn, id, &body) {
        Ok(true) => match db::get_contact(&conn, id) {
            Ok(Some(updated)) => Json(ApiResponse::ok(updated)),
            _ => Json(ApiResponse::err("Contact not found after update")),
        },
        Ok(false) => Json(ApiResponse::err("Contact not found")),
        Err(e)    => Json(ApiResponse::err(e.to_string())),
    }
}

#[delete("/api/contacts/<id>")]
fn api_delete_contact(
    db: &State<DbState>,
    id: i64,
) -> Json<ApiResponse<()>> {
    let conn = db.0.lock().unwrap();
    match db::delete_contact(&conn, id) {
        Ok(true)  => Json(ApiResponse::ok(())),
        Ok(false) => Json(ApiResponse::err("Contact not found")),
        Err(e)    => Json(ApiResponse::err(e.to_string())),
    }
}

#[get("/api/contacts/since/<since_id>")]
fn api_contacts_since(db: &State<DbState>, since_id: i64) -> Json<Vec<Contact>> {
    let conn = db.0.lock().unwrap();
    Json(db::get_contacts_since(&conn, since_id).unwrap_or_default())
}

#[get("/api/dupe?<call>&<band>&<mode>")]
fn api_dupe_check(
    db: &State<DbState>,
    call: &str,
    band: &str,
    mode: &str,
) -> Json<serde_json::Value> {
    let call = call.trim().to_uppercase();
    let conn = db.0.lock().unwrap();
    let is_dupe = db::is_dupe(&conn, &call, band, mode);
    Json(serde_json::json!({ "is_dupe": is_dupe }))
}

#[get("/api/sections")]
fn api_sections(db: &State<DbState>) -> Json<Vec<String>> {
    let conn = db.0.lock().unwrap();
    Json(db::get_worked_sections(&conn).unwrap_or_default())
}

// ── ADIF import ──────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct ImportResult {
    imported:        usize,
    skipped_dupe:    usize,
    skipped_invalid: usize,
    total:           usize,
}

#[post("/import/adif", data = "<data>")]
async fn import_adif(
    db:   &State<DbState>,
    data: Data<'_>,
) -> Json<ApiResponse<ImportResult>> {
    let content = match data.open(10.megabytes()).into_string().await {
        Ok(s) if s.is_complete() => s.into_inner(),
        Ok(_)  => return Json(ApiResponse::err("File too large (max 10 MB)")),
        Err(e) => return Json(ApiResponse::err(format!("Read error: {}", e))),
    };

    let entries = adif::parse(&content);
    let total   = entries.len();

    let conn = db.0.lock().unwrap();
    let (mut imported, mut skipped_dupe, mut skipped_invalid) = (0usize, 0usize, 0usize);

    for e in &entries {
        if db::is_dupe(&conn, &e.call, &e.band, &e.mode) {
            skipped_dupe += 1;
            continue;
        }
        let id      = n1mm::new_id();
        let contact = NewContact {
            call:     e.call.clone(),
            band:     e.band.clone(),
            mode:     e.mode.clone(),
            class:    e.class.clone(),
            section:  e.section.clone(),
            operator: e.operator.clone(),
        };
        match db::add_contact_with_time(&conn, &contact, &e.date, &e.time, Some(&id)) {
            Ok(_)  => imported += 1,
            Err(_) => skipped_invalid += 1,
        }
    }

    Json(ApiResponse::ok(ImportResult { imported, skipped_dupe, skipped_invalid, total }))
}

// ── ADIF export ──────────────────────────────────────────────────────────────

#[get("/export/adif")]
fn export_adif(db: &State<DbState>) -> Result<AdifDownload, String> {
    let conn = db.0.lock().unwrap();
    let config = db::get_site_config(&conn)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No site config found".to_string())?;
    let contacts = db::get_all_contacts_asc(&conn).map_err(|e| e.to_string())?;
    drop(conn);
    Ok(AdifDownload(build_adif(&config, &contacts)))
}

fn build_adif(config: &SiteConfig, contacts: &[Contact]) -> Vec<u8> {
    let mut out = String::new();
    out.push_str("Field Day Log\n");
    out.push_str("<ADIF_VER:5>3.1.4\n");
    out.push_str("<PROGRAMID:8>FDLogger\n");
    out.push_str("<EOH>\n\n");

    for c in contacts {
        let date = c.date.replace('-', "");
        let time = format!("{}00", c.time.replace(':', ""));
        let mode_adif = match c.mode.as_str() {
            "PH"  => "SSB",
            "CW"  => "CW",
            _     => "DIG",
        };
        let band_adif = c.band.to_lowercase();
        let f = |tag: &str, val: &str| format!("<{}:{}>{}", tag, val.len(), val);
        let record = format!(
            "{} {} {} {} {} {} {} {} {} {} <EOR>\n",
            f("CALL",             &c.call),
            f("MODE",             mode_adif),
            f("QSO_DATE",         &date),
            f("TIME_ON",          &time),
            f("BAND",             &band_adif),
            f("STATION_CALLSIGN", &config.callsign),
            f("CONTEST_ID",       "ARRL-FIELD-DAY"),
            f("CLASS",            &c.class),
            f("ARRL_SECT",        &c.section),
            f("OPERATOR",         &c.operator),
        );
        out.push_str(&record);
    }
    out.into_bytes()
}

// ── Launch ────────────────────────────────────────────────────────────────────

#[launch]
fn rocket() -> _ {
    let db_path = "fd_logger.db";
    let conn = Connection::open(db_path)
        .unwrap_or_else(|e| panic!("Failed to open database {}: {}", db_path, e));
    db::init_db(&conn)
        .unwrap_or_else(|e| panic!("Failed to initialize database: {}", e));

    println!("FD Logger starting on http://0.0.0.0:8000");
    println!("N1MM listener active on UDP :{}", n1mm::PORT);

    rocket::build()
        .manage(DbState(Mutex::new(conn)))
        .mount(
            "/",
            routes![
                index,
                setup_get,
                setup_post,
                logger,
                api_add_contact,
                api_update_contact,
                api_delete_contact,
                api_contacts_since,
                api_dupe_check,
                api_sections,
                import_adif,
                export_adif,
            ],
        )
        .attach(Template::fairing())
        .attach(N1mmFairing { db_path: db_path.to_string() })
}
