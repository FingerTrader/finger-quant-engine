//! Reviewed in-process reference catalog used by the CLI and equivalent library callers.
//! Registration is separate from generic compilation/evaluation; this is not plugin discovery.
use crate::{
    builtins,
    event_plan::Trigger,
    indicators,
    model::ModelError,
    registry::{Registry, RegistryBuilder},
};
use fqe_core::types::{NumericUnit, ScalarType, TypeDescriptor, ValueShape};
pub fn reference(trigger: &Trigger) -> Result<Registry, ModelError> {
    let mut builder = RegistryBuilder::new();
    builtins::register(&mut builder)?;
    if let Trigger::ClosedBar {
        instrument,
        price_scale,
        volume_scale,
        timeframe_ns,
    } = trigger
    {
        if *price_scale > 12 || *volume_scale > 18 || *timeframe_ns == 0 {
            return Err(crate::graph::error(
                "catalog_trigger",
                "$.trigger",
                "unsupported reference catalog trigger",
            ));
        }
        let price = TypeDescriptor::new(
            1,
            ValueShape::Scalar,
            ScalarType::Decimal {
                scale: *price_scale,
                unit: NumericUnit::Price(instrument.clone()),
            },
        )
        .map_err(|_| {
            crate::graph::error(
                "catalog_trigger",
                "$.trigger",
                "unsupported reference price type",
            )
        })?;
        indicators::register(&mut builder, "reference", price)?;
    }
    builder.freeze()
}
