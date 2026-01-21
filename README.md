# cal2

`cal2` is a small Rust CLI that fetches and lists public holidays with provider-backed data, renders colorized calendars, and lets you record personal days off alongside them. It supports standard `cal` command-line options for familiar usage.

![screenshot](screenshot.jpeg?raw=true)

## Features
- Standard `cal`-compatible CLI options (`-3`, `-y`, `-j`, `-h`, `-m`, `-A`, `-B`).
- Display the current quarter, a single month, or an entire year with ANSI colors.
- List every public holiday in the active year alongside its official name.
- Fetch official holidays from Argentina Datos (default) or OpenHolidays based on a country code.
- Configure a default country so you don't need `--country` every time.
- Cache holiday data per year and provider using XDG directories.
- Add or remove custom holidays for the active year from the command line.

## Installation

```bash
cargo install --path . --locked
```

You can run the binary directly with `cargo run -- <args>` while developing.

## Usage

### Calendar Display (cal-compatible)

```text
cal2 [-3hjy] [-A num] [-B num] [-m month] [[month] year]
```

| Option | Description |
|--------|-------------|
| `-h` | Turn off highlighting of today |
| `-j` | Display Julian days (day-of-year 1-365) |
| `-y` | Display full year calendar |
| `-3` | Display previous, current, and next month |
| `-m month` | Display specific month (1-12) |
| `-A num` | Display N months after current/specified month |
| `-B num` | Display N months before current/specified month |
| `year` | Display full year (e.g., `cal2 2024`) |
| `month year` | Display specific month (e.g., `cal2 6 2024`) |

Examples:

```bash
cal2                # Current quarter (default)
cal2 -3             # Previous, current, next month
cal2 -y             # Full year
cal2 2024           # Full year 2024
cal2 6 2024         # June 2024
cal2 -m 6           # June of current year
cal2 -j             # Julian day numbers
cal2 -h             # No today highlighting
cal2 -A 2           # Current month + 2 months after
cal2 -B 1 -A 1      # 3 months centered on current
cal2 -j -3 -h       # Combined flags
```

### Subcommands

```text
cal2 add <day> <month> [--description <TEXT>]
cal2 delete <day> <month>
cal2 list [--format table|json|markdown]
cal2 display [q|month|year]
cal2 config <set-country|clear-country|show>
```

The `--country <ISO>` flag works with all commands to override the default country.

### Holiday Management

- `cal2 list` – show all holidays for the current year.
- `cal2 list --format json` – emit the holiday list as JSON for scripting.
- `cal2 add --description "Family dinner" 24 12` – add December 24 with a custom label.
- `cal2 delete 6 1` – remove January 6 from the calendar.

### Configuration

Set a default country to avoid typing `--country` every time:

```bash
cal2 config set-country US    # Set default to US
cal2 config show              # Show current configuration
cal2 config clear-country     # Clear default (use built-in default)
```

The `config show` command displays:
- Config file path
- Data directory path
- Current default country setting

### Holiday Providers

- **Argentina Datos** is used when `--country` is omitted or set to `AR`. Data is fetched from `https://api.argentinadatos.com`.
- **OpenHolidays** is selected for any other ISO country code. Data comes from `https://openholidaysapi.org` in English, filtered to the requested year.

## File Locations

`cal2` follows the [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html):

| Purpose | Default Location |
|---------|------------------|
| Configuration | `$XDG_CONFIG_HOME/cal2/` or `~/.config/cal2/` |
| Cached Data | `$XDG_DATA_HOME/cal2/` or `~/.local/share/cal2/` |

Files:
- `~/.config/cal2/config.json` – stores default country setting
- `~/.local/share/cal2/hm-<year>` – cached holiday data for default provider
- `~/.local/share/cal2/hm-openholidays-<country>-<year>` – cached data for other countries

Removing cache files forces a fresh API fetch on next run.

### Custom Holidays

`cal2 add` and `cal2 delete` update the cache for the current year (based on your system clock). Custom dates are stored per provider, so you can maintain separate local overrides for multiple countries. When adding a date you can supply `--description` to store a custom name; if omitted, `cal2` records a generic label.

## Development

Run the tests before sending patches:

```bash
cargo test
```

To check code coverage locally:

```bash
cargo tarpaulin
```
