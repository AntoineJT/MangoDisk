//! Read-only engineering evidence for ordinary-user storage capability.
use mangodisk_platform::system_resources::{disk, disk_health};
use std::time::Instant;

fn main() {
    for volume in disk::list().expect("local volume catalogue") {
        let start = Instant::now();
        let result = disk_health::read(&volume);
        let outcome = match result {
            Ok(value) => serde_json::to_value(value).expect("serializable disk health"),
            Err(error) => {
                serde_json::json!({"unavailable":format!("{:?}",error.code()),"diagnostic":error.diagnostic()})
            }
        };
        println!(
            "{}",
            serde_json::json!({"volume":volume.name,"fileSystem":volume.file_system,"health":outcome,"elapsedUs":start.elapsed().as_micros()})
        );
    }
}
