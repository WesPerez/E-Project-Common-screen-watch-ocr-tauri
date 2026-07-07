use screen_watch_core::{
    config::WatchConfig,
    detect::{PreparedDetector, RgbFrame},
};
use serde_json::json;
use std::{env, path::PathBuf, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let config_path = PathBuf::from(
        args.next()
            .ok_or("usage: detect-config-frame <config.json> <frame.png> <template-base-dir>")?,
    );
    let frame_path = PathBuf::from(
        args.next()
            .ok_or("usage: detect-config-frame <config.json> <frame.png> <template-base-dir>")?,
    );
    let template_base_dir = PathBuf::from(
        args.next()
            .ok_or("usage: detect-config-frame <config.json> <frame.png> <template-base-dir>")?,
    );
    if args.next().is_some() {
        return Err("too many arguments".into());
    }

    let config = WatchConfig::from_path(&config_path)?;
    let frame = RgbFrame::from_image_path(&frame_path)?;
    let detector = PreparedDetector::from_config(&config, &template_base_dir)?;
    let started = Instant::now();
    let matches = detector.run(&frame);
    let elapsed_ms = started.elapsed().as_millis();

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": "pass",
            "configPath": config_path,
            "framePath": frame_path,
            "templateBaseDir": template_base_dir,
            "elapsedMs": elapsed_ms,
            "templateWorkers": detector.template_worker_limit(),
            "matches": matches,
        }))?
    );
    Ok(())
}
