//! nsm-attest attest --public-key KEY.der --output DOC: an attestation document from the Nitro Security Module (/dev/nsm) carrying KEY,
//! so KMS can encrypt its answer to the key only this sandbox holds (scripts/nitro/guest-init).
use aws_nitro_enclaves_nsm_api::api::{Request, Response};
use aws_nitro_enclaves_nsm_api::driver::{nsm_exit, nsm_init, nsm_process_request};
use serde_bytes::ByteBuf;
use std::{env, fs, process::exit};

fn main() {
    let a: Vec<String> = env::args().collect();
    let arg = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let (Some("attest"), Some(key), Some(out)) = (a.get(1).map(String::as_str), arg("--public-key"), arg("--output")) else {
        eprintln!("usage: nsm-attest attest --public-key FILE --output FILE"); exit(2)
    };
    let key = fs::read(&key).unwrap_or_else(|e| { eprintln!("{key}: {e}"); exit(1) });
    let fd = nsm_init();
    if fd < 0 { eprintln!("cannot open /dev/nsm"); exit(1) }
    let r = nsm_process_request(fd, Request::Attestation { user_data: None, nonce: None, public_key: Some(ByteBuf::from(key)) });
    nsm_exit(fd);
    match r {
        Response::Attestation { document } => fs::write(&out, document).unwrap_or_else(|e| { eprintln!("{out}: {e}"); exit(1) }),
        other => { eprintln!("NSM: {other:?}"); exit(1) }
    }
}
