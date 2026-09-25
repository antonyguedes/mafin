//! Gera a nota de corretagem SINTÉTICA dos testes E2E e a prévia calculada pelo parser real.
//!
//! `cargo run -p mafin --features test-support --example note_fixture -- <nota.pdf> <previa.json> <senha>`

use std::{env, fs, path::Path};

use base64::Engine;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let [pdf_path, json_path, password] = &args[..] else {
        panic!("uso: note_fixture <nota.pdf> <previa.json> <senha>");
    };
    let pdf = mafin_lib::import_testpdf::note_pdf(&mafin_lib::import_testpdf::sample_notes(), Some(password));
    let b64 = base64::engine::general_purpose::STANDARD.encode(&pdf);
    let preview = mafin_lib::import_preview_offline(&b64, Some(password)).expect("prévia da nota sintética");

    for path in [pdf_path, json_path] {
        if let Some(dir) = Path::new(path).parent() {
            fs::create_dir_all(dir).expect("falha ao criar diretório");
        }
    }
    fs::write(pdf_path, &pdf).expect("falha ao gravar PDF");
    let json = serde_json::json!({ "pdf_base64": b64, "password": password, "preview": preview });
    fs::write(json_path, serde_json::to_string_pretty(&json).unwrap()).expect("falha ao gravar JSON");
    eprintln!("nota sintética em {pdf_path}, prévia em {json_path}");
}
