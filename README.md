# FD Logger

A web-based contact logger for the ARRL Field Day amateur radio contest.
Runs as a standalone local web server — no internet connection required.
Multiple operators can log contacts simultaneously from any browser on the local network.

## Screenshots

### Setup Screen
![Setup Screen](screenshots/Setup%20Screen.png)

### Main Display – Dark Mode
![Main Display Dark Mode](screenshots/MainDisplay_DarkMode.png)

### Main Display – Light Mode
![Main Display Light Mode](screenshots/MainDisplay_LightMode.png)

### Main Display – With Contacts
![Main Display With Contacts](screenshots/MainDisplay_with_contacts.png)

## Features

- **Fast contact entry** — Tab cycles through Call / Class / Section; Enter submits
- **Duplicate checking** — warns on (call, band, mode) duplicates per Field Day scoring rules
- **ARRL / RAC sections grid** — all sections shown with worked / unworked status, tooltips, live count
- **ADIF import / export** — import existing logs; export for submission or third-party software
- **Accessibility** — ARIA landmarks, live regions, focus management, screen-reader announcements
- **Keyboard shortcuts**
  - `F1` — cycle band
  - `F2` — cycle mode (PH / CW / DIG)
  - `F3` — focus operator field
  - `F4` — edit last logged contact
  - `F5` — toggle fast-entry / navigation mode
  - `Esc` — clear the log entry form (fast-entry mode)
- **Dark / light theme** — toggle in the header; preference saved across sessions
- **Single binary** — SQLite is bundled; copy the binary and `templates/` directory to deploy

## External Integration

FD Logger stores all contacts in a plain SQLite database file (`fd_logger.db`). Integration
with external ham radio logging software is handled by separate bridge programs that read
and write this database directly. This keeps fd_logger simple and makes it straightforward
to add support for new programs without modifying the logger itself.

Bridge programs run alongside fd_logger as separate processes. The SQLite database uses
WAL mode so both processes can access it safely at the same time.

### Available bridges

| Bridge | Status | Description |
|--------|--------|-------------|
| [`n1mm_bridge`](https://github.com/kb3gtn/N1MM_BRIDGE) | Working | Full two-way sync with N1MM+ via TCP (port 12070) and UDP XML (port 12060) |

### n1mm_bridge

[`n1mm_bridge`](https://github.com/kb3gtn/N1MM_BRIDGE) provides full bidirectional
contact synchronisation with N1MM+ stations on the local network:

- Contacts logged in **N1MM+** appear immediately in the FD Logger web UI
- Contacts logged in **FD Logger** are sent to all connected N1MM+ stations within one second
- N1MM+ shows FD Logger as a healthy peer (Send OK / Receive OK) in its network status

Run it from the same directory as `fd_logger.db`:

```bash
./n1mm_bridge \
  --callsign KB3GTN \
  --station WA3NAN-REMOTE \
  --local-ip 192.168.1.17
```

See the [N1MM Bridge repository](https://github.com/kb3gtn/N1MM_BRIDGE) for full
build instructions, all options, and protocol details.

## Requirements

- Rust 1.75 or later
- Cargo

## Building

```bash
git clone git@github.com:kb3gtn/FD_LOGGER.git
cd FD_LOGGER
cargo build --release
```

The release binary is written to `target/release/fd_logger`.

## Running

```bash
# From the project directory (templates/ must be alongside the binary)
./target/release/fd_logger
```

The server starts on `http://0.0.0.0:8000`. Open a browser on any machine on the
local network and navigate to `http://<host-ip>:8000`.

On first launch you will be prompted to enter your station callsign, Field Day class,
and section. This information is stored in the database and included in all exported logs.

## Deployment

Copy these two items to any directory and run the binary from there:

```
fd_logger          (binary)
templates/         (HTML templates directory)
```

The SQLite database (`fd_logger.db`) is created automatically in the working directory
on first run.

## Bands supported

160M / 80M / 40M / 20M / 15M / 10M / 6M / 2M / 70CM

## Modes supported

| Log entry | ADIF export |
|-----------|-------------|
| PH        | SSB         |
| CW        | CW          |
| DIG       | DIG         |

## ADIF Import

Contacts are imported from `.adi` / `.adif` files via the **⬆ Import ADIF** button in
the header. Duplicates on (call, band, mode) are skipped automatically.

Fields read from ADIF: `CALL`, `BAND`, `MODE`, `QSO_DATE`, `TIME_ON`, `CLASS`,
`ARRL_SECT`, `SRX_STRING`, `OPERATOR`.

## License

GNU General Public License v3.0 — see [LICENSE](LICENSE) for details.
