use std::{
    collections::HashMap,
    fs::File,
    io::{BufWriter, Write},
};

use serde::Deserialize;

use crate::HM;

#[derive(Debug, Deserialize)]
struct Resp {
    fecha: String,
}

pub fn get_filename(year: i32) -> String {
    shellexpand::tilde(&format!("~/.config/hm-{year}")).to_string()
}

pub fn load(fname: &String) -> Result<HM, ()> {
    let cache = File::open(fname);
    match cache {
        Ok(reader) => match bincode::deserialize_from::<File, HM>(reader) {
            Ok(resp) => Ok(resp),
            Err(_) => Err(()),
        },
        Err(_) => Err(()),
    }
}
pub fn save(fname: &String, hm: &HM) {
    let file = File::create(fname).unwrap();
    let mut writer = BufWriter::new(file);
    bincode::serialize_into(&mut writer, hm).unwrap();
    writer.flush().unwrap();
}

pub fn get_holidays(year: i32) -> HM {
    let fname = get_filename(year);
    if let Ok(hm) = load(&fname) {
        return hm;
    }
    let data = reqwest::blocking::get(format!("https://api.argentinadatos.com/v1/feriados/{year}"))
        .unwrap()
        .text()
        .unwrap();
    let v: Vec<Resp> = serde_json::from_str(&data).unwrap();
    let mut hm = HashMap::new();
    for hday in v {
        let mut fecha = hday.fecha.splitn(3, '-');
        fecha.next();
        let month: u32 = fecha.next().unwrap().parse().unwrap();
        let day: u32 = fecha.next().unwrap().parse().unwrap();
        let k = (day, month);
        let v = true;
        hm.insert(k, v);
    }
    save(&fname, &hm);
    hm
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, fs, time::SystemTime};

    fn temp_file(label: &str) -> String {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("cal2-{label}-{nanos}"));
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn load_returns_err_for_missing_file() {
        let fname = temp_file("missing");
        assert!(load(&fname).is_err());
    }

    #[test]
    fn save_and_load_roundtrip_preserves_holidays() {
        let mut hm = HashMap::new();
        hm.insert((1, 1), true);
        hm.insert((25, 12), true);

        let fname = temp_file("roundtrip");
        save(&fname, &hm);
        let loaded = load(&fname).expect("load should succeed after save");

        assert_eq!(loaded, hm);
        fs::remove_file(&fname).unwrap();
    }

    #[test]
    fn get_filename_places_cache_under_config_directory() {
        let year = 2030;
        let fname = get_filename(year);
        assert!(
            fname.ends_with(&format!("hm-{year}")),
            "unexpected cache filename: {fname}"
        );
    }
}
