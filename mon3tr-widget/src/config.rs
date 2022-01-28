use anyhow::Result;
use serde::{Deserialize, Serialize};
use winit::event::VirtualKeyCode;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnimationItem {
    pub name: String,
    #[serde(rename = "loop", default, skip_serializing_if = "is_false")]
    pub loop_: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<f32>,
}

fn is_false(loop_: &bool) -> bool {
    !loop_
}

fn is_true(loop_: &bool) -> bool {
    *loop_
}

fn default_return_to_idle() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Action {
    pub trigger: VirtualKeyCode,
    pub sequence: Vec<AnimationItem>,
    #[serde(default = "default_return_to_idle", skip_serializing_if = "is_true")]
    pub return_to_idle: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// List of actions that can be triggered by input
    pub actions: Vec<Action>,
    /// Animation to play on idle
    pub idle_animation: Option<String>,
    #[serde(default = "default_initial_size")]
    pub window_size: (f64, f64),
    #[serde(default)]
    pub window_position: (f64, f64),
    #[serde(default = "default_scale")]
    pub scale: f32,
    #[serde(default = "default_bottom_offset")]
    pub bottom_offset: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SavedState {}

fn default_initial_size() -> (f64, f64) {
    (300.0, 400.0)
}

fn default_scale() -> f32 {
    1.0
}

fn default_bottom_offset() -> f32 {
    5.0
}

pub fn load(path: &str) -> Result<Config> {
    let file = std::fs::File::open(path)?;
    let config: Config = serde_yaml::from_reader(file)?;
    Ok(config)
}

pub fn save(config: &Config, path: &str) -> Result<()> {
    let file = std::fs::File::create(path)?;
    serde_yaml::to_writer(file, config)?;
    Ok(())
}
