use fqe_core::types::{
    Evaluation, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape,
};
use fqe_ir::{
    expressions::{
        BinaryOp, CrossDirection, Crossing, Expr, ExpressionDraft, ExpressionInstance,
        ExpressionPlan,
    },
    model::Port,
};
use std::sync::Arc;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let u64_type = TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::U64)?;
    let bool_type = TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::Bool)?;
    let plan = Arc::new(ExpressionPlan::new(ExpressionDraft {
        inputs: vec![
            Port {
                name: "left".into(),
                value_type: u64_type.clone(),
            },
            Port {
                name: "right".into(),
                value_type: u64_type.clone(),
            },
            Port {
                name: "guard".into(),
                value_type: bool_type.clone(),
            },
        ],
        crossings: vec![Crossing {
            id: "cross".into(),
            direction: CrossDirection::Above,
            left: Expr::Input("left".into()),
            right: Expr::Input("right".into()),
        }],
        outputs: vec![(
            "decision".into(),
            Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(Expr::Input("guard".into())),
                right: Box::new(Expr::Cross("cross".into())),
            },
        )],
    })?);
    let mut run = ExpressionInstance::new(plan, "demo.run", "demo.instance")?;
    for (seq, left, guard) in [
        (1, 1, false),
        (2, 3, false),
        (3, 4, true),
        (4, 2, true),
        (5, 3, true),
    ] {
        let values = vec![
            (
                "left".into(),
                Evaluation::Ready(TypedValue::new(u64_type.clone(), ScalarValue::U64(left))?),
            ),
            (
                "right".into(),
                Evaluation::Ready(TypedValue::new(u64_type.clone(), ScalarValue::U64(2))?),
            ),
            (
                "guard".into(),
                Evaluation::Ready(TypedValue::new(
                    bool_type.clone(),
                    ScalarValue::Bool(guard),
                )?),
            ),
        ];
        let out = run.evaluate(seq, values)?;
        println!(
            "step={seq} left={left} guard={guard} decision={:?}",
            out.outputs["decision"]
        );
    }
    println!(
        "accepted_steps={}; subscribed crossing advances behind false guards",
        run.accepted_steps()
    );
    Ok(())
}
