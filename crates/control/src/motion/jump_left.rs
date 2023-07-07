use color_eyre::Result;
use context_attribute::context;
use filtering::low_pass_filter::LowPassFilter;
use framework::MainOutput;
use hardware::PathsInterface;
use motionfile::{MotionFile, MotionInterpolator};
use nalgebra::Vector2;
use types::{
    parameters::{FallProtection, FallStateEstimation, Jump},
    BodyJoints, ConditionInput, CycleTime, HeadJoints, Joints, JointsCommand, MotionSafeExits,
    MotionSelection, MotionType, SensorData,
};

pub struct JumpLeft {
    interpolator: MotionInterpolator<Joints<f32>>,
    roll_pitch_filter: LowPassFilter<Vector2<f32>>,
}

#[context]
pub struct CreationContext {
    pub fall_state_estimation: Parameter<FallStateEstimation, "fall_state_estimation">,
    pub hardware_interface: HardwareInterface,
    pub motion_safe_exits: PersistentState<MotionSafeExits, "motion_safe_exits">,
}

#[context]
pub struct CycleContext {
    pub motion_safe_exits: PersistentState<MotionSafeExits, "motion_safe_exits">,

    pub condition_input: Input<ConditionInput, "condition_input">,
    pub cycle_time: Input<CycleTime, "cycle_time">,
    pub motion_selection: Input<MotionSelection, "motion_selection">,
    pub sensor_data: Input<SensorData, "sensor_data">,
    pub fall_protection: Parameter<FallProtection, "fall_protection">,
    pub jump: Parameter<Jump, "jump">,
}

#[context]
#[derive(Default)]
pub struct MainOutputs {
    pub jump_left_joints_command: MainOutput<JointsCommand<f32>>,
}

impl JumpLeft {
    pub fn new(context: CreationContext<impl PathsInterface>) -> Result<Self> {
        let paths = context.hardware_interface.get_paths();
        Ok(Self {
            interpolator: MotionFile::from_path(paths.motions.join("jump_left.json"))?
                .try_into()?,
            roll_pitch_filter: LowPassFilter::with_smoothing_factor(
                Vector2::zeros(),
                context.fall_state_estimation.roll_pitch_low_pass_factor,
            ),
        })
    }

    pub fn cycle(&mut self, context: CycleContext) -> Result<MainOutputs> {
        let last_cycle_duration = context.cycle_time.last_cycle_duration;

        self.roll_pitch_filter
            .update(context.sensor_data.inertial_measurement_unit.roll_pitch);

        if context.motion_selection.current_motion == MotionType::JumpLeft {
            self.interpolator
                .advance_by(last_cycle_duration, context.condition_input);
        } else {
            self.interpolator.reset();
        }

        context.motion_safe_exits[MotionType::JumpLeft] = self.interpolator.is_finished();

        let stiffnesses = if self.roll_pitch_filter.state().y.abs()
            > context.jump.jump_ground_impact_angular_threshold
        {
            Joints::from_head_and_body(
                HeadJoints::fill(context.jump.jump_ground_impact_head_stiffness),
                BodyJoints::fill(context.jump.jump_ground_impact_body_stiffness),
            )
        } else {
            Joints::from_head_and_body(
                HeadJoints::fill(0.8),
                BodyJoints::fill_mirrored(
                    context.fall_protection.arm_stiffness,
                    context.fall_protection.leg_stiffness,
                ),
            )
        };

        Ok(MainOutputs {
            jump_left_joints_command: JointsCommand {
                positions: self.interpolator.value(),
                stiffnesses: stiffnesses,
            }
            .into(),
        })
    }
}
