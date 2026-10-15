use serde_json::json;
fn main() {
    for (run, version) in [("baseline", 1), ("change", 2)] {
        println!(
            "{}",
            json!({"protocol_version":1,"request_id":run,"type":"snapshot","source":"simulator.software","scope":"simulator-machine","run_id":run,"observed_at":"2026-09-10T00:00:00Z","complete":true,"items":[{"id":"demo-app","data":{"name":"Demonstration App","version":version}}]})
        );
    }
}
