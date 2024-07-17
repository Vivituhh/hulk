use std::sync::Arc;

use communication::messages::TextOrBinary;
use eframe::egui::Widget;
use gilrs::{Axis, Button, Gamepad, GamepadId, Gilrs};
use serde_json::{json, Value};
use types::step_plan::Step;

use crate::{nao::Nao, panel::Panel};

pub struct RemotePanel {
    nao: Arc<Nao>,
    gilrs: Gilrs,
    active_gamepad: Option<GamepadId>,
    enabled: bool,
}

impl Panel for RemotePanel {
    const NAME: &'static str = "Remote";

    fn new(nao: Arc<Nao>, _value: Option<&Value>) -> Self {
        let gilrs = Gilrs::new().expect("could not initialize gamepad library");
        let active_gamepad = None;
        let enabled = false;

        Self {
            nao,
            gilrs,
            active_gamepad,
            enabled,
        }
    }

    fn save(&self) -> Value {
        json!({})
    }
}

fn get_axis_value(gamepad: Gamepad, axis: Axis) -> Option<f32> {
    Some(gamepad.axis_data(axis)?.value())
}

impl RemotePanel {
    fn update_step(&self, step: Value) {
        self.nao.write(
            "parameters.step_planner.injected_step",
            TextOrBinary::Text(step),
        )
    }

    fn update_look_at_angle(&self, angle: Value) {
        self.nao.write(
            "parameters.look_around.injected_head_angle",
            TextOrBinary::Text(angle),
        )
    }
}

impl Widget for &mut RemotePanel {
    fn ui(self, ui: &mut eframe::egui::Ui) -> eframe::egui::Response {
        const MAXIMUM_HEAD_ANGLE: f32 = 1.0;

        self.gilrs.inc();

        if ui.checkbox(&mut self.enabled, "Enabled (Start)").changed() {
            self.update_step(Value::Null);
            self.update_look_at_angle(Value::Null);
        };

        while let Some(event) = self.gilrs.next_event() {
            if let gilrs::EventType::ButtonPressed(Button::Start, _) = event.event {
                self.enabled = !self.enabled;
                if !self.enabled {
                    self.update_step(Value::Null)
                }
            };
            self.active_gamepad = Some(event.id);
        }

        if let Some(gamepad) = self.active_gamepad.map(|id| self.gilrs.gamepad(id)) {
            let right = get_axis_value(gamepad, Axis::LeftStickX).unwrap_or(0.0);
            let forward = get_axis_value(gamepad, Axis::LeftStickY).unwrap_or(0.0);
            let turn_right = get_axis_value(gamepad, Axis::RightStickX).unwrap_or(0.0);

            let left = -right;
            let turn = -turn_right;

            let right_rotation = gamepad
                .button_data(Button::RightTrigger2)
                .map(|button| button.value())
                .unwrap_or_default();
            let left_rotation = gamepad
                .button_data(Button::LeftTrigger2)
                .map(|button| button.value())
                .unwrap_or_default();

            let injected_angle = (right_rotation - left_rotation) * MAXIMUM_HEAD_ANGLE;

            let step = Step {
                forward,
                left,
                turn,
            };

            if self.enabled {
                self.update_step(serde_json::to_value(step).unwrap());
                self.update_look_at_angle(serde_json::to_value(injected_angle).unwrap());
            }

            ui.vertical(|ui| {
                let label_1 = ui.label(&format!("{:#?}", step));
                let label_2 = ui.label(&format!("Head angle: {}", injected_angle));

                label_1.union(label_2)
            })
            .inner
        } else {
            ui.label("No controller found")
        }
    }
}
