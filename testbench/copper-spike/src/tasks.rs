use cu29::bincode::{Decode, Encode};
use cu29::prelude::{
    ComponentConfig, ComponentId, CopperListView, CuComponentState, CuContext, CuError, CuMonitor,
    CuMonitoringMetadata, CuMonitoringRuntime, CuMsg, CuResult, CuSinkTask, CuSrcTask, CuTask,
    Decision, Freezable, Reflect,
};
use cu29::{input_msg, output_msg};
use hefaos_testbench_contracts::{SubjectConfigV0, SubjectInputV0, SubjectOutputV0};
use hefaos_testbench_reference::{ReferenceSubject, ReferenceSubjectSnapshotV0};
use serde::{Deserialize, Serialize};

use crate::resources::RunCounter;

/// Test-only fault modes carried on the experimental Copper wire.  They are not
/// part of a `HefaOS` contract and are deliberately unavailable to callers of the
/// transport-neutral `Subject` trait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Encode, Decode, Reflect)]
pub enum CopperTaskFault {
    Error,
    Panic,
}

/// The experimental graph fails closed when a Copper task or simulated step
/// reports an error.  This keeps a bounded timing sentinel from becoming an
/// ignored infinite generated run, and matches the subject's terminal-fault
/// behavior.
pub struct FailClosedMonitor;

impl CuMonitor for FailClosedMonitor {
    fn new(_metadata: CuMonitoringMetadata, _runtime: CuMonitoringRuntime) -> CuResult<Self> {
        Ok(Self)
    }

    fn process_copperlist(&self, _ctx: &CuContext, _view: CopperListView<'_>) -> CuResult<()> {
        Ok(())
    }

    fn process_error(
        &self,
        _component_id: ComponentId,
        _step: CuComponentState,
        _error: &CuError,
    ) -> Decision {
        Decision::Shutdown
    }
}

/// Copper-owned wire payload.  The durable testbench contract deliberately
/// remains Copper-free; JSON conversion is only at this experimental edge.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode, Reflect)]
pub struct WireTurn {
    pub config_json: Option<Vec<u8>>,
    pub input_json: Vec<u8>,
    pub fault: Option<CopperTaskFault>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode, Reflect)]
pub struct WireOutput {
    pub output_json: Vec<u8>,
}

#[allow(dead_code)]
#[derive(Reflect)]
pub struct InjectedSource;

impl Freezable for InjectedSource {}

impl CuSrcTask for InjectedSource {
    type Resources<'r> = ();
    type Output<'m> = output_msg!(WireTurn);

    fn new(_config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self> {
        Ok(Self)
    }

    fn process(&mut self, _ctx: &CuContext, _output: &mut Self::Output<'_>) -> CuResult<()> {
        Err(CuError::from(
            "source injection must be supplied by the simulation callback",
        ))
    }
}

/// The graph's controller task is the only place that evaluates a semantic
/// turn.  The adapter never calls `ReferenceSubject::step` itself: Copper's
/// typed controller output is returned by the sink observation below.
#[derive(Reflect)]
#[reflect(from_reflect = false)]
pub struct SemanticController {
    #[reflect(ignore)]
    semantic: ReferenceSubject,
    last_index: u64,
    #[reflect(ignore)]
    snapshot_json: Vec<u8>,
}

impl Freezable for SemanticController {
    fn freeze<E: cu29::bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), cu29::bincode::error::EncodeError> {
        Encode::encode(&self.last_index, encoder)?;
        let snapshot_json = serde_json::to_vec(&self.semantic.snapshot())
            .map_err(|_| cu29::bincode::error::EncodeError::Other("encode semantic snapshot"))?;
        Encode::encode(&snapshot_json, encoder)
    }

    fn thaw<D: cu29::bincode::de::Decoder>(
        &mut self,
        decoder: &mut D,
    ) -> Result<(), cu29::bincode::error::DecodeError> {
        self.last_index = Decode::decode(decoder)?;
        self.snapshot_json = Decode::decode(decoder)?;
        let snapshot: ReferenceSubjectSnapshotV0 = serde_json::from_slice(&self.snapshot_json)
            .map_err(|error| {
                cu29::bincode::error::DecodeError::OtherString(format!(
                    "decode semantic snapshot: {error}"
                ))
            })?;
        self.semantic.restore(&snapshot).map_err(|error| {
            cu29::bincode::error::DecodeError::OtherString(format!(
                "restore semantic snapshot: {error}"
            ))
        })?;
        Ok(())
    }
}

impl CuTask for SemanticController {
    type Resources<'r> = ControllerResources;
    type Input<'m> = input_msg!(WireTurn);
    type Output<'m> = output_msg!(WireOutput);

    fn new(_config: Option<&ComponentConfig>, resources: Self::Resources<'_>) -> CuResult<Self> {
        let controller_resources::Resources { counter: _counter } = resources;
        Ok(Self {
            semantic: ReferenceSubject::new(),
            last_index: 0,
            snapshot_json: Vec::new(),
        })
    }

    fn process(
        &mut self,
        _ctx: &CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let Some(turn) = input.payload() else {
            output.clear_payload();
            return Ok(());
        };
        match turn.fault {
            Some(CopperTaskFault::Error) => {
                return Err(CuError::from(
                    "injected Copper SemanticController task error",
                ));
            }
            Some(CopperTaskFault::Panic) => {
                panic!("injected Copper SemanticController task panic");
            }
            None => {}
        }
        if let Some(config_json) = &turn.config_json {
            let config: SubjectConfigV0 = serde_json::from_slice(config_json)
                .map_err(|error| CuError::from(format!("decode reset config: {error}")))?;
            self.semantic
                .reset(&config)
                .map_err(|error| CuError::from(format!("reset semantic controller: {error}")))?;
            self.last_index = 0;
        }
        let semantic_input: SubjectInputV0 = serde_json::from_slice(&turn.input_json)
            .map_err(|error| CuError::from(format!("decode semantic input: {error}")))?;
        let semantic_output: SubjectOutputV0 = self
            .semantic
            .step(&semantic_input)
            .map_err(|error| CuError::from(format!("process semantic controller: {error}")))?;
        let output_json = serde_json::to_vec(&semantic_output)
            .map_err(|error| CuError::from(format!("encode semantic output: {error}")))?;
        output.set_payload(WireOutput { output_json });
        self.last_index = self.last_index.saturating_add(1);
        Ok(())
    }
}

#[derive(Reflect)]
pub struct RecordingSink;

impl Freezable for RecordingSink {}

impl CuSinkTask for RecordingSink {
    type Resources<'r> = ();
    type Input<'m> = input_msg!(WireOutput);

    fn new(_config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self> {
        Ok(Self)
    }

    fn process(&mut self, _ctx: &CuContext, input: &Self::Input<'_>) -> CuResult<()> {
        if input.payload().is_none() {
            return Err(CuError::from(
                "recording sink received an empty semantic turn",
            ));
        }
        Ok(())
    }
}

mod controller_resources {
    use super::RunCounter;
    use cu29::resources;

    resources!({ counter => Owned<RunCounter> });
}

type ControllerResources = controller_resources::Resources;
