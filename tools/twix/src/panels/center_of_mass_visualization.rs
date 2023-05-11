use std::{str::FromStr, sync::Arc};

use communication::client::CyclerOutput;
use eframe::egui::{
    plot::{Line, Plot, PlotBounds, PlotPoints},
    Response, Ui,
};
use log::error;
use nalgebra::{Point2, Point3};

use crate::{nao::Nao, panel::Panel, value_buffer::ValueBuffer};

pub struct CenterOfMassVisualizationPanel {
    value_buffer: ValueBuffer,
    nao: Arc<Nao>,
}

impl Panel for CenterOfMassVisualizationPanel {
    const NAME: &'static str = "Center of Mass";

    fn new(nao: Arc<Nao>, _value: Option<&serde_json::Value>) -> Self {
        let output = CyclerOutput::from_str("Control.main_outputs.center_of_mass").unwrap();
        let value_buffer = nao.subscribe_output(output);

        CenterOfMassVisualizationPanel { value_buffer, nao }
    }
}

impl CenterOfMassVisualizationPanel {
    pub fn ui(&self, ui: &mut Ui) -> Response {
        let center_of_mass: Result<Point3<f32>, _> = match self.value_buffer.get_latest() {
            Ok(value) => match serde_json::from_value(value) {
                Ok(center_of_mass) => Ok(center_of_mass),
                Err(error) => Err(error.to_string()),
            },
            Err(error) => Err(error.to_string()),
        };

        match center_of_mass {
            Ok(center_of_mass) => {
                let end = center_of_mass.xy();

                let points = PlotPoints::from_iter([[0.0, 0.0], [end.x as f64, end.y as f64]]);
                Plot::new(ui.id().with("center_of_mass"))
                    .view_aspect(1.0)
                    .show(ui, |plot_ui| {
                        plot_ui.set_plot_bounds(PlotBounds::from_min_max([-0.2, -0.2], [0.2, 0.2]));
                        plot_ui.line(Line::new(points))
                    })
                    .response
            }
            Err(error) => ui.label(error),
        }
    }
}
