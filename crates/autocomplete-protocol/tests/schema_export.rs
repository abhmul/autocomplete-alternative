use autocomplete_protocol::schema;
use serde_json::Value;

#[test]
fn schema_export_writes_request_and_response_schema_files() {
    let tempdir = tempfile::tempdir().expect("temp dir");

    let written = schema::export_schema_files(tempdir.path()).expect("schemas export");

    let names: Vec<_> = written
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        [
            "autocomplete-request.v1.schema.json",
            "autocomplete-response.v1.schema.json"
        ]
    );

    for path in written {
        let text = std::fs::read_to_string(&path).expect("schema file is readable");
        let json: Value = serde_json::from_str(&text).expect("schema file contains json");
        jsonschema::validator_for(&json).expect("exported schema compiles");
    }
}
