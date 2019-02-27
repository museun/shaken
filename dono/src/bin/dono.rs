use serde::Deserialize;
use std::io::Read;

use dono::server::{Item, ItemKind};

fn main() {
    #[allow(dead_code)]
    #[derive(Deserialize)]
    pub struct Response<'a> {
        ts: u64,
        url: &'a str,
        title: &'a str,
    }

    let mut stdin = std::io::stdin();

    let mut size = [0u8; 4];
    stdin.read_exact(&mut size).unwrap();
    let n: u32 = unsafe { std::mem::transmute(size) }; // (* int*)size

    let mut buf = vec![0; n as usize];
    stdin.read_exact(&mut buf).unwrap();

    let val = serde_json::from_slice::<Response>(&buf).unwrap();

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap();

    let data = match serde_json::to_string(&Item {
        kind: ItemKind::Youtube(val.url.to_string()),
        ts: (ts.as_secs() * 1000 + u64::from(ts.subsec_nanos()) / 1_000_000) as i64,
        version: 1,
    }) {
        Ok(data) => data,
        Err(err) => {
            eprintln!("error making item: {}", err);
            std::process::exit(1)
        }
    };

    // TODO pull this from the env or something
    if let Some(err) = ureq::post("http://localhost:50006/youtube")
        .send_string(&data)
        .synthetic_error()
    {
        eprintln!("{}: {}", err.status(), err.status_text());
        std::process::exit(1)
    }

    println!("okay");
}
