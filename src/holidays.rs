use std::{
    collections::HashMap,
    fs::File,
    io::{BufWriter, Write},
};

use serde::Deserialize;

use crate::HM;

#[derive(Debug, Deserialize)]
struct Resp {
    dia: u32,
    mes: u32,
}

pub fn get_filename(year: i32) -> String {
    shellexpand::tilde(&format!("~/.config/hm-{year}")).to_string()
}

pub fn load(fname: &String) -> Result<HM, ()> {
    let cache = File::open(&fname);
    match cache {
        Ok(reader) => match bincode::deserialize_from::<File, HM>(reader) {
            Ok(resp) => Ok(resp),
            Err(_) => Err(()),
        },
        Err(_) => Err(()),
    }
}
pub fn save(fname: &String, hm: &HM) {
    let file = File::create(&fname).unwrap();
    let mut writer = BufWriter::new(file);
    bincode::serialize_into(&mut writer, hm).unwrap();
    writer.flush().unwrap();
}

pub fn get_holidays(year: i32) -> HM {
    let fname = get_filename(year);
    match load(&fname) {
        Ok(hm) => {
            return hm;
        }
        Err(_) => {}
    }
    let data = reqwest::blocking::get(format!(
        "https://nolaborables.com.ar/api/v2/feriados/{year}"
    ))
    .unwrap()
    .text()
    .unwrap();
    let v: Vec<Resp> = serde_json::from_str(&data).unwrap();
    let mut hm = HashMap::new();
    for day in v {
        let k = (day.dia, day.mes);
        let v = true;
        hm.insert(k, v);
    }
    save(&fname, &hm);
    hm
}
