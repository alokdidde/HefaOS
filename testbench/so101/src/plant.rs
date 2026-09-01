use hefaos_testbench_contracts::{
    AppliedActuationV0, ClockEpoch, DurationNs, PINNED_SO101_MODEL_DIGEST, SensorFrameV0, Sequence,
    So101StateV0, SourceEpoch, Tick, VirtualTimeNs,
};
use hefaos_testbench_harness::{Plant, PlantError};

#[cfg(feature = "mujoco")]
use crate::MujocoSo101Plant;
use crate::{MockSo101Plant, So101PlantError};

impl Plant for MockSo101Plant {
    fn id(&self) -> &'static str {
        self.plant_id()
    }

    fn model_digest(&self) -> &'static str {
        PINNED_SO101_MODEL_DIGEST
    }

    fn reset(&mut self, initial: &So101StateV0) -> Result<(), PlantError> {
        self.reset_state(initial)
            .map_err(|error| invalid_initial_state(&error))
    }

    fn observe(
        &self,
        _tick: Tick,
        now: VirtualTimeNs,
        clock_epoch: ClockEpoch,
        source_epoch: SourceEpoch,
        sequence: Sequence,
    ) -> Result<SensorFrameV0, PlantError> {
        Ok(self.sensor_frame(now, clock_epoch, source_epoch, sequence))
    }

    fn apply(&mut self, actuation: &AppliedActuationV0) -> Result<(), PlantError> {
        self.apply_actuation(actuation)
            .map_err(|error| invalid_actuation(&error))
    }

    fn advance(&mut self, duration: DurationNs) -> Result<(), PlantError> {
        self.advance_duration(duration)
            .map_err(|error| backend(&error))
    }

    fn state(&self) -> Result<So101StateV0, PlantError> {
        Ok(MockSo101Plant::state(self))
    }
}

#[cfg(feature = "mujoco")]
impl Plant for MujocoSo101Plant {
    fn id(&self) -> &'static str {
        self.plant_id()
    }

    fn model_digest(&self) -> &'static str {
        PINNED_SO101_MODEL_DIGEST
    }

    fn reset(&mut self, initial: &So101StateV0) -> Result<(), PlantError> {
        self.reset_state(initial)
            .map_err(|error| invalid_initial_state(&error))
    }

    fn observe(
        &self,
        _tick: Tick,
        now: VirtualTimeNs,
        clock_epoch: ClockEpoch,
        source_epoch: SourceEpoch,
        sequence: Sequence,
    ) -> Result<SensorFrameV0, PlantError> {
        Ok(self.sensor_frame(now, clock_epoch, source_epoch, sequence))
    }

    fn apply(&mut self, actuation: &AppliedActuationV0) -> Result<(), PlantError> {
        self.apply_actuation(actuation)
            .map_err(|error| invalid_actuation(&error))
    }

    fn advance(&mut self, duration: DurationNs) -> Result<(), PlantError> {
        self.advance_duration(duration)
            .map_err(|error| backend(&error))
    }

    fn state(&self) -> Result<So101StateV0, PlantError> {
        Ok(MujocoSo101Plant::state(self))
    }
}

fn invalid_initial_state(error: &So101PlantError) -> PlantError {
    PlantError::InvalidInitialState(error.to_string())
}

fn invalid_actuation(error: &So101PlantError) -> PlantError {
    PlantError::InvalidActuation(error.to_string())
}

fn backend(error: &So101PlantError) -> PlantError {
    PlantError::Backend(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_uses_the_typed_plant_boundary() {
        let mut plant = MockSo101Plant::default();
        Plant::reset(&mut plant, &So101StateV0::zero()).unwrap();
        let frame = Plant::observe(
            &plant,
            Tick(2),
            VirtualTimeNs(10_000_000),
            ClockEpoch(3),
            SourceEpoch(4),
            Sequence(5),
        )
        .unwrap();

        assert_eq!(Plant::id(&plant), "so101-mock");
        assert_eq!(frame.clock_epoch, ClockEpoch(3));
        assert_eq!(frame.source_epoch, SourceEpoch(4));
        assert_eq!(frame.sequence, Sequence(5));
    }

    #[test]
    fn mock_maps_invalid_reset_to_a_typed_harness_error() {
        let mut plant = MockSo101Plant::default();
        let mut state = So101StateV0::zero();
        state.gripper_position = f64::NAN;

        let error = Plant::reset(&mut plant, &state).unwrap_err();
        assert!(matches!(error, PlantError::InvalidInitialState(_)));
    }
}
