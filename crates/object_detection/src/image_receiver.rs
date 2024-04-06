use std::time::{Duration, SystemTime};

use color_eyre::Result;
use context_attribute::context;
use framework::{AdditionalOutput, MainOutput};
use hardware::{CameraInterface, TimeInterface};
use serde::{Deserialize, Serialize};
use types::{camera_position::CameraPosition, ycbcr422_image::YCbCr422Image};

#[derive(Deserialize, Serialize)]
pub struct ImageReceiver {
    last_cycle_start: SystemTime,
    image_count: u32,
    last_frame_time: SystemTime,
}

#[context]
pub struct CreationContext {
    hardware_interface: HardwareInterface,
}

#[context]
pub struct CycleContext {
    hardware_interface: HardwareInterface,
    camera_position: Parameter<CameraPosition, "image_receiver.$cycler_instance.camera_position">,
    frame_rate_limit: Parameter<u32, "image_receiver.$cycler_instance.frame_rate_limit">,

    last_cycle_time: AdditionalOutput<Duration, "cycle_time">,
    image_waiting_time: AdditionalOutput<Duration, "image_waiting_time">,
}

#[context]
pub struct MainOutputs {
    pub image: MainOutput<Option<YCbCr422Image>>,
}

impl ImageReceiver {
    pub fn new(context: CreationContext<impl TimeInterface>) -> Result<Self> {
        Ok(Self {
            last_cycle_start: context.hardware_interface.get_now(),
            image_count: 0,
            last_frame_time: context.hardware_interface.get_now(),
        })
    }

    pub fn cycle(
        &mut self,
        mut context: CycleContext<impl CameraInterface + TimeInterface>,
    ) -> Result<MainOutputs> {
        let duration_since_last_cycle = context
            .hardware_interface
            .get_now()
            .duration_since(self.last_frame_time)?;
        let minimum_elapsed_duration =
            Duration::from_secs_f64(1.0 / *context.frame_rate_limit as f64);
        if duration_since_last_cycle.cmp(&minimum_elapsed_duration) == std::cmp::Ordering::Less {
            return Ok(MainOutputs { image: None.into() });
        } else {
            self.last_frame_time = context.hardware_interface.get_now();
        }

        let now = context.hardware_interface.get_now();

        context.last_cycle_time.fill_if_subscribed(|| {
            now.duration_since(self.last_cycle_start)
                .expect("time ran backwards")
        });
        let earlier = context.hardware_interface.get_now();
        let image = context
            .hardware_interface
            .read_from_camera(*context.camera_position)?;

        context.image_waiting_time.fill_if_subscribed(|| {
            context
                .hardware_interface
                .get_now()
                .duration_since(earlier)
                .expect("time ran backwards")
        });
        self.last_cycle_start = context.hardware_interface.get_now();

        Ok(MainOutputs {
            image: Some(image).into(),
        })
    }
}
