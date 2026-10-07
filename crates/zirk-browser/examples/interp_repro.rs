use zirk_browser::contract;

fn main() {
    for path in std::env::args().skip(1) {
        let source = std::fs::read_to_string(&path).unwrap();
        let response = zirk_browser::run(contract::Request {
            api_version: 1,
            trace: None,
            source,
            virtual_name: path.clone(),
            limits: None,
        });
        println!("=== {path}");
        for d in &response.diagnostics {
            println!("[diag {} {}] {}", d.severity, d.code, d.message);
        }
        for event in &response.output {
            print!("{}", event.text);
        }
        match (&response.completion, &response.failure) {
            (_, Some(f)) => println!("[failure: {:?} {} {}]", f.kind, f.code, f.message),
            (Some(c), _) => println!("[completion: {c:?}]"),
            _ => println!("[no completion/failure]"),
        }
    }
}
