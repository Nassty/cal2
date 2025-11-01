# cal2

`cal2` is a small Rust CLI that fetches and lists public holidays with provider-backed data, renders colorized calendars, and lets you record personal days off alongside them.

## Features
- Display the current quarter, a single month, or an entire year with ANSI colors.
- List every public holiday in the active year alongside its official name.
- Fetch official holidays from Argentina Datos (default) or OpenHolidays based on a country code.
- Cache holiday data per year and provider under `~/.config/` so repeated runs are instant.
- Add or remove custom holidays for the active year from the command line.

## Installation

```bash
cargo install --path .
```

You can run the binary directly with `cargo run -- <args>` while developing.

## Usage

```text
cal2 add [--country <ISO>] <day> <month>
cal2 delete [--country <ISO>] <day> <month>
cal2 list [--country <ISO>]
cal2 display [--country <ISO>] [q|month|year]
```

Every command validates its inputs and emits a descriptive error (non-zero exit
code) if something goes wrong, such as network failures or malformed
arguments.

`cal2 list` accepts `--format table|json|markdown` (default `table`) to control
its output style.

Common examples:

- `cal2 display` – render the current quarter as a colorized calendar (default command).
- `cal2 display --country DE year` – view all German months using OpenHolidays.
- `cal2 list` – show all holidays for the current year from Argentina Datos.
- `cal2 list --country US` – fetch the current year's US holidays via OpenHolidays.
- `cal2 list --format json` – emit the holiday list as JSON for scripting.
- `cal2 add 24 12` – add December 24 to your personal list for the active year.
- `cal2 delete --country DE 6 1` – drop Epiphany from a German calendar you generated earlier.

### Holiday Providers

- **Argentina Datos** is used when `--country` is omitted or set to `AR`. Data is fetched from `https://api.argentinadatos.com`.
- **OpenHolidays** is selected for any other ISO country code. Data comes from `https://openholidaysapi.org` in English, filtered to the requested year.

Holiday results are stored in binary caches named `hm-<provider>-<year>` inside `~/.config/`. Removing those files forces a fresh API fetch.

### Custom Holidays

`cal2 add` and `cal2 delete` update the cache for the current year (based on your system clock). Custom dates are stored per provider, so you can maintain separate local overrides for multiple countries.

## Development

Run the tests before sending patches:

```bash
cargo test
```

To check code coverage locally:

```bash
cargo tarpaulin
```

If you need to inspect cached data while hacking on the project, check the files in `~/.config/` starting with `hm-`.
