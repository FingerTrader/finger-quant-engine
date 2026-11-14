//! Example/test driver: a compiled pure plan projected into the P1-05 evaluator.
//! Incremental plans require P1-08; this is one logical expression evaluation.
use fqe_core::types::{Evaluation, TypedValue};
use fqe_ir::{compiler::CompiledPlan, expressions::ExpressionInstance};
use std::{collections::BTreeMap, sync::Arc};
type Values = BTreeMap<String, Evaluation<TypedValue>>;
pub fn evaluate(plan: &CompiledPlan, inputs: Values) -> Result<Values, Box<dyn std::error::Error>> {
    let expressions = plan.pure_expressions()?;
    let mut instance = ExpressionInstance::new(Arc::new(expressions), "compiled.example", "one")?;
    Ok(instance.evaluate(1, inputs.into_iter().collect())?.outputs)
}
