use agm::core::config::AgmConfig;
use divan::{Bencher, black_box};

fn main() {
    divan::main();
}

/// Builds a config holding `size` skills and MCPs.
fn sample_config(size: usize) -> AgmConfig {
    let mut config = AgmConfig::new();
    config.skills = (0..size).map(|index| format!("skill-{index}")).collect();
    config.mcps = (0..size).map(|index| format!("mcp-{index}")).collect();
    config
}

/// Renders a config as the JSON that would be stored in `agm.json`.
fn sample_json(size: usize) -> String {
    serde_json::to_string_pretty(&sample_config(size)).unwrap()
}

#[divan::bench]
fn new() -> AgmConfig {
    AgmConfig::new()
}

/// Mirrors the write path of `init_config`, without touching the filesystem.
#[divan::bench(args = [0, 32, 512])]
fn serialize(bencher: Bencher, size: usize) {
    let config = sample_config(size);

    bencher.bench(|| {
        let mut buffer = Vec::new();
        serde_json::to_writer_pretty(&mut buffer, black_box(&config)).unwrap();
        buffer
    });
}

/// Mirrors the read path of `load_config`, without touching the filesystem.
#[divan::bench(args = [0, 32, 512])]
fn deserialize(bencher: Bencher, size: usize) {
    let json = sample_json(size);

    bencher.bench(|| serde_json::from_str::<AgmConfig>(black_box(&json)).unwrap());
}
