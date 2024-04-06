use std::time::{Duration, SystemTime, UNIX_EPOCH};

use color_eyre::Result;
use context_attribute::context;
use coordinate_systems::{Field, Ground};
use filtering::tap_detector::TapDetector;
use framework::MainOutput;
use hardware::NetworkInterface;
use linear_algebra::Isometry2;
use serde::{Deserialize, Serialize};
use spl_network_messages::{HulkMessage, PlayerNumber};
use types::{
    buttons::Buttons, cycle_time::CycleTime, messages::OutgoingMessage, sensor_data::SensorData,
};

#[derive(Deserialize, Serialize)]
pub struct ButtonFilter {
    chest_button_tap_detector: TapDetector,
    head_buttons_touched: SystemTime,
    last_head_buttons_touched: bool,
    calibration_buttons_touched: SystemTime,
    last_calibration_buttons_touched: bool,
    first_head_button_touched: SystemTime,
    last_first_head_button_touched: bool,
}

#[context]
pub struct CreationContext {}

#[context]
pub struct CycleContext {
    sensor_data: Input<SensorData, "sensor_data">,
    cycle_time: Input<CycleTime, "cycle_time">,

    calibration_buttons_timeout: Parameter<Duration, "button_filter.calibration_buttons_timeout">,
    head_buttons_timeout: Parameter<Duration, "button_filter.head_buttons_timeout">,

    time_to_reach_kick_position: CyclerState<Duration, "time_to_reach_kick_position">,

    hardware: HardwareInterface,
}

#[context]
#[derive(Default)]
pub struct MainOutputs {
    pub buttons: MainOutput<Buttons>,
}

impl ButtonFilter {
    pub fn new(_context: CreationContext) -> Result<Self> {
        Ok(Self {
            chest_button_tap_detector: TapDetector::default(),
            head_buttons_touched: UNIX_EPOCH,
            last_head_buttons_touched: false,
            calibration_buttons_touched: UNIX_EPOCH,
            last_calibration_buttons_touched: false,
            first_head_button_touched: UNIX_EPOCH,
            last_first_head_button_touched: false,
        })
    }

    pub fn cycle(&mut self, context: CycleContext<impl NetworkInterface>) -> Result<MainOutputs> {
        let head_buttons_timeout = *context.head_buttons_timeout;
        let calibration_buttons_timeout = *context.calibration_buttons_timeout;
        let touch_sensors = &context.sensor_data.touch_sensors;

        self.chest_button_tap_detector
            .update(touch_sensors.chest_button);

        let head_buttons_touched =
            touch_sensors.head_front && touch_sensors.head_middle && touch_sensors.head_rear;

        let head_buttons_touched_initially =
            head_buttons_touched && !self.last_head_buttons_touched;
        if head_buttons_touched_initially {
            self.head_buttons_touched = context.cycle_time.start_time;
        }
        self.last_head_buttons_touched = head_buttons_touched;

        let debounced_head_buttons_touched = head_buttons_touched
            && context
                .cycle_time
                .start_time
                .duration_since(self.head_buttons_touched)
                .unwrap()
                >= head_buttons_timeout;

                

        let first_head_button_touched = touch_sensors.head_front && !touch_sensors.head_middle && !touch_sensors.head_rear;

        let first_head_button_touched_initially =
            first_head_button_touched && !self.last_first_head_button_touched;

        if first_head_button_touched_initially {
            self.first_head_button_touched = context.cycle_time.start_time;
        }
        self.last_first_head_button_touched = first_head_button_touched;

        let debounced_first_head_button_touched = first_head_button_touched
            && context
                .cycle_time
                .start_time
                .duration_since(self.first_head_button_touched)
                .unwrap()
                >= head_buttons_timeout;

        if debounced_first_head_button_touched {
            context
                .hardware
                .write_to_network(OutgoingMessage::Spl(HulkMessage {
                    player_number: PlayerNumber::Seven,
                    fallen: false,
                    pose: Isometry2::<Ground, Field>::identity().as_pose(),
                    over_arms_pose_detected: true,
                    ball_position: None,
                    time_to_reach_kick_position: Some(*context.time_to_reach_kick_position),
                }))?;
            println!("Initial -> Ready")
        }

        let calibration_buttons_touched = touch_sensors.chest_button && touch_sensors.head_front;

        let calibration_buttons_touched_initially =
            calibration_buttons_touched && !self.last_calibration_buttons_touched;
        if calibration_buttons_touched_initially {
            self.calibration_buttons_touched = context.cycle_time.start_time;
        }
        self.last_calibration_buttons_touched = calibration_buttons_touched;

        let debounced_calibration_buttons_touched = calibration_buttons_touched
            && context
                .cycle_time
                .start_time
                .duration_since(self.calibration_buttons_touched)
                .unwrap()
                >= calibration_buttons_timeout;

        Ok(MainOutputs {
            buttons: Buttons {
                is_chest_button_pressed: self.chest_button_tap_detector.is_single_tapped(),
                head_buttons_touched: debounced_head_buttons_touched,
                calibration_buttons_touched: debounced_calibration_buttons_touched,
            }
            .into(),
        })
    }
}
