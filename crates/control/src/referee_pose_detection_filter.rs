use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
    time::{Duration, SystemTime},
};

use color_eyre::Result;
use context_attribute::context;
use coordinate_systems::{Field, Ground};
use framework::{AdditionalOutput, MainOutput, PerceptionInput};
use hardware::NetworkInterface;
use linear_algebra::Isometry2;
use serde::{Deserialize, Serialize};
use spl_network_messages::{HulkMessage, PlayerNumber};
use types::{
    cycle_time::CycleTime,
    fall_state::FallState,
    messages::{IncomingMessage, OutgoingMessage},
    players::Players,
    pose_kinds::PoseKind,
};

#[derive(Deserialize, Serialize)]
pub struct RefereePoseDetectionFilter {
    detection_times: Players<Option<SystemTime>>,
    own_detected_above_arm_poses_queue: VecDeque<bool>,
}

#[context]
pub struct CreationContext {
    referee_pose_queue_length:
        Parameter<usize, "object_detection.object_detection_top.referee_pose_queue_length">,
}

#[context]
pub struct CycleContext {
    hardware_interface: HardwareInterface,

    time_to_reach_kick_position: CyclerState<Duration, "time_to_reach_kick_position">,

    network_message: PerceptionInput<IncomingMessage, "SplNetwork", "message">,
    detected_referee_pose_kind:
        PerceptionInput<Option<PoseKind>, "ObjectDetectionTop", "detected_referee_pose_kind?">,
    cycle_time: Input<CycleTime, "cycle_time">,
    fall_state: Input<FallState, "fall_state">,

    initial_message_grace_period:
        Parameter<Duration, "referee_pose_detection_filter.initial_message_grace_period">,
    minimum_above_head_arms_detections:
        Parameter<usize, "referee_pose_detection_filter.minimum_above_head_arms_detections">,
    player_number: Parameter<PlayerNumber, "player_number">,

    player_referee_detection_times:
        AdditionalOutput<Players<Option<SystemTime>>, "player_referee_detection_times">,

    referee_pose_queue_length:
        Parameter<usize, "object_detection.object_detection_top.referee_pose_queue_length">,
    minimum_number_poses_before_message: Parameter<
        usize,
        "object_detection.object_detection_top.minimum_number_poses_before_message",
    >,

    referee_pose_queue: AdditionalOutput<VecDeque<bool>, "referee_pose_queue">,
}

#[context]
#[derive(Default)]
pub struct MainOutputs {
    pub is_referee_initial_pose_detected: MainOutput<bool>,
}

impl RefereePoseDetectionFilter {
    pub fn new(context: CreationContext) -> Result<Self> {
        Ok(Self {
            detection_times: Default::default(),
            own_detected_above_arm_poses_queue: VecDeque::with_capacity(
                *context.referee_pose_queue_length,
            ),
        })
    }

    pub fn cycle(
        &mut self,
        mut context: CycleContext<impl NetworkInterface>,
    ) -> Result<MainOutputs> {
        let cycle_start_time = context.cycle_time.start_time;

        self.update(&context)?;

        let is_referee_initial_pose_detected = decide(
            self.detection_times,
            cycle_start_time,
            *context.initial_message_grace_period,
            *context.minimum_above_head_arms_detections,
        );

        context
            .player_referee_detection_times
            .fill_if_subscribed(|| self.detection_times);

        context
            .referee_pose_queue
            .fill_if_subscribed(|| self.own_detected_above_arm_poses_queue.clone());

        Ok(MainOutputs {
            is_referee_initial_pose_detected: is_referee_initial_pose_detected.into(),
        })
    }

    fn update(&mut self, context: &CycleContext<impl NetworkInterface>) -> Result<()> {
        let time_tagged_persistent_messages =
            unpack_message_tree(&context.network_message.persistent);

        for (time, message) in time_tagged_persistent_messages {
            if message.is_referee_ready_signal_detected {
                self.detection_times[message.player_number] = Some(time);
            }
        }

        let own_detected_pose_times =
            unpack_own_detection_tree(&context.detected_referee_pose_kind.persistent);

        for (time, detection) in own_detected_pose_times {
            if detection == PoseKind::AboveHeadArms {
                self.own_detected_above_arm_poses_queue.push_front(true);
                self.detection_times[*context.player_number] = Some(time);
            }
        }
        self.own_detected_above_arm_poses_queue
            .truncate(*context.referee_pose_queue_length);

        let detected_referee_pose_count = self
            .own_detected_above_arm_poses_queue
            .iter()
            .filter(|x| **x)
            .count();
        if detected_referee_pose_count >= *context.minimum_number_poses_before_message {
            send_own_detection_message(
                context.hardware_interface.clone(),
                *context.player_number,
                *context.fall_state,
                *context.time_to_reach_kick_position,
            )?;
        }

        Ok(())
    }
}

fn decide(
    pose_detection_times: Players<Option<SystemTime>>,
    cycle_start_time: SystemTime,
    initial_message_grace_period: Duration,
    minimum_above_head_arms_detections: usize,
) -> bool {
    let detected_above_head_arms_poses = pose_detection_times
        .iter()
        .filter(|(_, detection_time)| match detection_time {
            Some(detection_time) => is_in_grace_period(
                cycle_start_time,
                *detection_time,
                initial_message_grace_period,
            ),
            None => false,
        })
        .count();
    detected_above_head_arms_poses >= minimum_above_head_arms_detections
}

fn is_in_grace_period(
    cycle_start_time: SystemTime,
    earlier_time: SystemTime,
    grace_period: Duration,
) -> bool {
    cycle_start_time
        .duration_since(earlier_time)
        .expect("Time ran backwards")
        < grace_period
}

fn unpack_message_tree(
    message_tree: &BTreeMap<SystemTime, Vec<&IncomingMessage>>,
) -> BTreeMap<SystemTime, HulkMessage> {
    message_tree
        .iter()
        .flat_map(|(time, messages)| messages.iter().map(|message| (*time, message)))
        .filter_map(|(time, message)| match message {
            IncomingMessage::GameController(_, _) => None,
            IncomingMessage::Spl(message) => Some((time, *message)),
        })
        .collect()
}

fn unpack_own_detection_tree(
    pose_kind_tree: &BTreeMap<SystemTime, Vec<Option<&PoseKind>>>,
) -> BTreeMap<SystemTime, PoseKind> {
    pose_kind_tree
        .iter()
        .flat_map(|(time, pose_kinds)| pose_kinds.iter().map(|&pose_kind| (*time, pose_kind)))
        .filter_map(|(time, pose_kind)| Some(time).zip(pose_kind.cloned()))
        .collect()
}

fn send_own_detection_message<T: NetworkInterface>(
    hardware_interface: Arc<T>,
    player_number: PlayerNumber,
    fall_state: FallState,
    time_to_reach_kick_position: Duration,
) -> Result<()> {
    hardware_interface.write_to_network(OutgoingMessage::Spl(HulkMessage {
        player_number,
        fallen: matches!(fall_state, FallState::Fallen { .. }),
        pose: Isometry2::<Ground, Field>::default().as_pose(),
        is_referee_ready_signal_detected: true,
        ball_position: None,
        time_to_reach_kick_position: Some(time_to_reach_kick_position),
    }))
}
