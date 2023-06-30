use color_eyre::Result;
use context_attribute::context;
use framework::{AdditionalOutput, MainOutput};
use nalgebra::Point2;
use spl_network_messages::Team;
use std::f32::consts::PI;
use types::{
    configuration::Behavior, FieldDimensions, GameControllerState, PathObstacle, PathSegment,
    WorldState,
};

use crate::behavior::walk_to_pose::WalkPathPlanner;

#[context]

pub struct CreationContext {}

#[context]
pub struct CycleContext {
    pub world_state: Input<WorldState, "world_state">,
    pub field_dimensions: Parameter<FieldDimensions, "field_dimensions">,
    pub configuration: Parameter<Behavior, "behavior">,
    pub path_obstacles: AdditionalOutput<Vec<PathObstacle>, "time_to_reach_obstacles">,
}

#[context]
#[derive(Default)]
pub struct MainOutputs {
    pub dribble_path: MainOutput<Option<Vec<PathSegment>>>,
}

pub struct DribblePath {}
impl DribblePath {
    pub fn new(_context: CreationContext) -> Result<Self> {
        Ok(Self {})
    }

    pub fn cycle(&mut self, mut context: CycleContext) -> Result<MainOutputs> {
        let world_state = context.world_state;
        let field_dimensions = context.field_dimensions;
        let configuration = &context.configuration.path_planning;
        let parameters = &context.configuration.dribbling;
        let path_obstacles_output = &mut context.path_obstacles;
        let walk_path_planner =
            WalkPathPlanner::new(field_dimensions, &world_state.obstacles, configuration);
        let kick_decisions = match world_state.kick_decisions.as_ref() {
            Some(it) => it,
            None => return Ok(MainOutputs::default()),
        };
        let Some(best_kick_decision) = kick_decisions.first() else { return Ok(MainOutputs::default()) };
        let ball_position = match world_state.ball {
            Some(ball_position) => ball_position,
            None => return Ok(MainOutputs::default()),
        }
        .ball_in_ground;
        let best_pose = best_kick_decision.kick_pose;
        let Some(robot_to_field) = world_state.robot.robot_to_field else { return Ok(MainOutputs::default()) };
        let robot_to_ball = ball_position.coords;
        let dribble_pose_to_ball = ball_position.coords - best_pose.translation.vector;
        let angle = robot_to_ball.angle(&dribble_pose_to_ball);
        let should_avoid_ball = angle > parameters.angle_to_approach_ball_from_threshold;
        let ball_obstacle = should_avoid_ball.then_some(ball_position);
        let ball_obstacle_radius_factor = (angle
            - parameters.angle_to_approach_ball_from_threshold)
            / (PI - parameters.angle_to_approach_ball_from_threshold);

        let is_near_ball = matches!(
            world_state.ball,
            Some(ball) if ball.ball_in_ground.coords.norm() < parameters.ignore_robot_when_near_ball_radius,
        );
        let obstacles = if is_near_ball {
            &[]
        } else {
            world_state.obstacles.as_slice()
        };

        let rule_obstacles = if matches!(
            world_state.game_controller_state,
            Some(GameControllerState {
                kicking_team: Team::Hulks,
                ..
            })
        ) {
            &[]
        } else {
            world_state.rule_obstacles.as_slice()
        };
        let path = walk_path_planner.plan(
            best_pose * Point2::origin(),
            robot_to_field,
            ball_obstacle,
            ball_obstacle_radius_factor,
            obstacles,
            rule_obstacles,
            path_obstacles_output,
        );
        Ok(MainOutputs {
            dribble_path: Some(path).into(),
        })
    }
}