use std::time::{Duration, Instant};

use anyhow::Result;
use spine::{AnimationState, AnimationStateData, Atlas, Skeleton, SkeletonData};

use crate::config::Config;

pub struct SpineState {
    _atlas: Atlas,
    _skel_data: SkeletonData,
    _anim_state_data: AnimationStateData,

    pub skel: Skeleton,
    pub anim: AnimationState,

    last_render: Option<Instant>,
}

impl SpineState {
    pub fn new(config: &Config) -> Result<Self> {
        let atlas = Atlas::new(&format!("{}??/char.atlas", config.pack))?;
        let skel_data =
            SkeletonData::new_binary(&atlas, &format!("{}??/char.skel", config.pack), 1.0)?;
        let anim_data = AnimationStateData::new(&skel_data, 0.0)?;

        let mut skel = Skeleton::new(&skel_data)?;
        skel.set_x(0.0);
        skel.set_y(0.0);

        let mut anim = AnimationState::new(&anim_data)?;
        if let Some(ref idle_name) = config.idle_animation {
            anim.set_animation_by_name(0, idle_name, true);
        }

        Ok(Self {
            _atlas: atlas,
            _skel_data: skel_data,
            _anim_state_data: anim_data,

            skel,
            anim,

            last_render: None,
        })
    }

    pub fn prepare_render(&mut self) {
        let now = Instant::now();
        let delta = if let Some(last_render) = self.last_render {
            now - last_render
        } else {
            Duration::from_millis(0)
        }
        .as_secs_f32();
        self.last_render = Some(now);

        self.anim.update(delta);
        self.skel.apply_animation(&self.anim);
        self.skel.update_world_transform();
    }
}
