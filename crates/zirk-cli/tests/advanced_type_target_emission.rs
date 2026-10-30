use std::path::PathBuf;
use zirk_codegen_llvm::compile_to_object_for_triple;
use zirk_diagnostics::DiagnosticSink;

#[test]
fn generated_json_codec_emits_for_all_nine_targets() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "zirk-generated-json-codec-target-matrix-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create fixture directory");
    let entry = root.join("main.zrk");
    std::fs::write(
        &entry,
        r#"import { JSONCodec } from std.json;
class CodecLabel implements JSONCodec<CodecLabel> {
    value: String;
    construct(value: String) { this.value = value; }
}
fn main(): Void {
    try {
        inmut source: CodecLabel = CodecLabel("target-matrix");
        inmut decoded: CodecLabel = CodecLabel.decode_json(CodecLabel.encode_json(source));
    } catch JsonConversionError(_) { }
}
main();
"#,
    )
    .expect("write codec fixture");

    let mut sink = DiagnosticSink::new();
    let frontend = zirk_cli::frontend::run_frontend(&entry, &mut sink).unwrap_or_else(|| {
        panic!(
            "generated-codec fixture failed to check:\n{}",
            sink.render(zirk_diagnostics::RenderStyle::Human)
        );
    });
    let ir = zirk_ir::lower(&frontend.program, &frontend.checked);
    zirk_ir::verify(&ir).expect("generated-codec IR must be valid");

    for target in zirk_codegen_llvm::TARGETS {
        let object = root.join(PathBuf::from(format!("{}.o", target.name)));
        compile_to_object_for_triple(
            &ir,
            "generated_json_codec_target_matrix",
            target.triple,
            &object,
        )
        .unwrap_or_else(|error| panic!("{}: {}", target.name, error.render()));
        let bytes = std::fs::read(&object).expect("read emitted codec object");
        assert!(!bytes.is_empty(), "{} object is empty", target.name);
        std::fs::remove_file(object).expect("remove emitted codec object");
    }
    std::fs::remove_dir_all(root).expect("remove fixture directory");
}
