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
