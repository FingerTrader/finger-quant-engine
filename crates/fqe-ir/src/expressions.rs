//! Bounded typed expressions and transactional crossing subscriptions.
//! No serialized expression codec, whole-model compiler or market scheduler.
use crate::{
    model::{MAX_PORTS, ModelError, Port, identifier},
    registry::{CallContext, PreparedCall},
};
use fqe_core::{
    numeric::{NumericError, Rounding},
    types::{
        Evaluation, NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
type Result<T> = std::result::Result<T, ModelError>;
pub const MAX_EXPRESSION_DEPTH: usize = 32;
pub const MAX_EXPRESSION_NODES: usize = 4096;
pub const MAX_CROSSINGS: usize = 256;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply(Rounding),
    Divide(Rounding),
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}
#[derive(Clone)]
pub enum Expr {
    Literal(TypedValue),
    Input(String),
    Cross(String),
    Not(Box<Expr>),
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        call: Arc<PreparedCall>,
        inputs: Vec<(String, Expr)>,
        output: String,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrossDirection {
    Above,
    Below,
}
#[derive(Clone)]
pub struct Crossing {
    pub id: String,
    pub direction: CrossDirection,
    pub left: Expr,
    pub right: Expr,
}
#[derive(Clone)]
pub struct ExpressionDraft {
    pub inputs: Vec<Port>,
    pub crossings: Vec<Crossing>,
    pub outputs: Vec<(String, Expr)>,
}
pub struct ExpressionPlan {
    draft: ExpressionDraft,
    output_types: Vec<Port>,
    cross_order: Vec<String>,
}
fn error(code: &'static str, path: &str, detail: &str) -> ModelError {
    ModelError::new(code, path, detail)
}
fn bool_type() -> TypeDescriptor {
    TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::Bool).expect("supported Boolean")
}
fn boolean(v: bool) -> TypedValue {
    TypedValue::new(bool_type(), ScalarValue::Bool(v)).expect("supported Boolean")
}
fn is_bool(t: &TypeDescriptor) -> bool {
    t.scalar() == &ScalarType::Bool
}
fn numeric(t: &TypeDescriptor) -> bool {
    matches!(
        t.scalar(),
        ScalarType::I64 | ScalarType::U64 | ScalarType::Decimal { .. }
    )
}
fn operation_type(
    op: BinaryOp,
    left: &TypeDescriptor,
    right: &TypeDescriptor,
    path: &str,
) -> Result<TypeDescriptor> {
    if left != right {
        return Err(error(
            "expression_type",
            path,
            "operands require identical types/units/scales",
        ));
    }
    match op {
        BinaryOp::And | BinaryOp::Or if is_bool(left) => Ok(bool_type()),
        BinaryOp::Equal | BinaryOp::NotEqual => Ok(bool_type()),
        BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual
            if numeric(left) =>
        {
            Ok(bool_type())
        }
        BinaryOp::Add | BinaryOp::Subtract if numeric(left) => Ok(left.clone()),
        BinaryOp::Multiply(rounding) | BinaryOp::Divide(rounding) if numeric(left) => {
            match left.scalar() {
                ScalarType::Decimal {
                    unit: NumericUnit::Scalar | NumericUnit::Ratio,
                    ..
                } => {}
                ScalarType::I64 | ScalarType::U64 if rounding == Rounding::Exact => {}
                _ => {
                    return Err(error(
                        "expression_units",
                        path,
                        "multiply/divide require dimensionless operands; integer policy must be Exact",
                    ));
                }
            }
            Ok(left.clone())
        }
        _ => Err(error(
            "expression_type",
            path,
            "operator is unsupported for operand type",
        )),
    }
}
struct Checker<'a> {
    inputs: &'a BTreeMap<String, TypeDescriptor>,
    crossings: &'a BTreeSet<String>,
    nodes: usize,
    text_bytes: usize,
}
impl Checker<'_> {
    fn check(
        &mut self,
        e: &Expr,
        depth: usize,
        path: &str,
        deps: &mut BTreeSet<String>,
    ) -> Result<TypeDescriptor> {
        self.nodes += 1;
        if depth > MAX_EXPRESSION_DEPTH || self.nodes > MAX_EXPRESSION_NODES {
            return Err(error(
                "expression_limit",
                path,
                "expression depth/node budget exceeded",
            ));
        }
        match e {
            Expr::Literal(v) => {
                if let ScalarValue::Text(s) = v.value() {
                    self.text_bytes += s.len();
                    if self.text_bytes > 262144 {
                        return Err(error(
                            "expression_limit",
                            path,
                            "literal text budget exceeded",
                        ));
                    }
                }
                Ok(v.descriptor().clone())
            }
            Expr::Input(k) => {
                identifier(k, path)?;
                self.inputs
                    .get(k)
                    .cloned()
                    .ok_or_else(|| error("expression_reference", path, "undeclared input"))
            }
            Expr::Cross(k) => {
                identifier(k, path)?;
                if !self.crossings.contains(k) {
                    return Err(error("expression_reference", path, "undeclared crossing"));
                }
                deps.insert(k.clone());
                Ok(bool_type())
            }
            Expr::Not(v) => {
                let t = self.check(v, depth + 1, path, deps)?;
                if !is_bool(&t) {
                    return Err(error("expression_type", path, "not requires Boolean"));
                }
                Ok(t)
            }
            Expr::Binary { op, left, right } => {
                let l = self.check(left, depth + 1, path, deps)?;
                let r = self.check(right, depth + 1, path, deps)?;
                operation_type(*op, &l, &r, path)
            }
            Expr::Call {
                call,
                inputs,
                output,
            } => {
                let d = call.descriptor().declaration();
                if inputs.len() != d.inputs.len() {
                    return Err(error(
                        "expression_arity",
                        path,
                        "call input count differs from descriptor",
                    ));
                }
                let mut names = BTreeSet::new();
                for (k, e) in inputs {
                    if !names.insert(k) {
                        return Err(error("expression_duplicate", path, "duplicate call input"));
                    }
                    let p = d
                        .inputs
                        .iter()
                        .find(|p| p.name == *k)
                        .ok_or_else(|| error("expression_arity", path, "unknown call input"))?;
                    if self.check(e, depth + 1, path, deps)? != p.value_type {
                        return Err(error(
                            "expression_type",
                            path,
                            "call input differs from descriptor",
                        ));
                    }
                }
                d.outputs
                    .iter()
                    .find(|p| p.name == *output)
                    .map(|p| p.value_type.clone())
                    .ok_or_else(|| error("expression_reference", path, "call output unavailable"))
            }
        }
    }
}
impl ExpressionPlan {
    pub fn new(mut draft: ExpressionDraft) -> Result<Self> {
        if draft.inputs.len() > MAX_PORTS
            || draft.outputs.is_empty()
            || draft.outputs.len() > MAX_PORTS
            || draft.crossings.len() > MAX_CROSSINGS
        {
            return Err(error(
                "expression_limit",
                "$",
                "invalid input/output/crossing count",
            ));
        }
        let mut inputs = BTreeMap::new();
        for p in &draft.inputs {
            identifier(&p.name, "$.inputs")?;
            if inputs
                .insert(p.name.clone(), p.value_type.clone())
                .is_some()
            {
                return Err(error("expression_duplicate", "$.inputs", "duplicate input"));
            }
        }
        let mut crossings = BTreeSet::new();
        for c in &draft.crossings {
            identifier(&c.id, "$.crossings")?;
            if !crossings.insert(c.id.clone()) {
                return Err(error(
                    "expression_duplicate",
                    "$.crossings",
                    "duplicate crossing",
                ));
            }
        }
        let mut checker = Checker {
            inputs: &inputs,
            crossings: &crossings,
            nodes: 0,
            text_bytes: 0,
        };
        let mut dependencies = BTreeMap::new();
        for c in &draft.crossings {
            let path = format!("$.crossings.{}", c.id);
            let mut deps = BTreeSet::new();
            let left = checker.check(&c.left, 0, &path, &mut deps)?;
            let right = checker.check(&c.right, 0, &path, &mut deps)?;
            if left != right || !numeric(&left) {
                return Err(error(
                    "expression_type",
                    &path,
                    "crossing needs identical numeric operands",
                ));
            }
            dependencies.insert(c.id.clone(), deps);
        }
        let mut names = BTreeSet::new();
        let mut output_types = Vec::new();
        for (name, e) in &draft.outputs {
            identifier(name, "$.outputs")?;
            if !names.insert(name) {
                return Err(error(
                    "expression_duplicate",
                    "$.outputs",
                    "duplicate output",
                ));
            }
            let t = checker.check(e, 0, &format!("$.outputs.{name}"), &mut BTreeSet::new())?;
            output_types.push(Port {
                name: name.clone(),
                value_type: t,
            });
        }
        let mut cross_order = Vec::new();
        let mut ready = BTreeSet::new();
        while !dependencies.is_empty() {
            let id = dependencies
                .iter()
                .find(|(_, deps)| deps.is_subset(&ready))
                .map(|(id, _)| id.clone())
                .ok_or_else(|| {
                    error(
                        "expression_cycle",
                        "$.crossings",
                        "cyclic crossing dependencies",
                    )
                })?;
            dependencies.remove(&id);
            ready.insert(id.clone());
            cross_order.push(id);
        }
        draft.inputs.sort_by(|a, b| a.name.cmp(&b.name));
        draft.crossings.sort_by(|a, b| a.id.cmp(&b.id));
        draft.outputs.sort_by(|a, b| a.0.cmp(&b.0));
        output_types.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Self {
            draft,
            output_types,
            cross_order,
        })
    }
    pub fn declaration(&self) -> &ExpressionDraft {
        &self.draft
    }
    pub fn output_types(&self) -> &[Port] {
        &self.output_types
    }
    pub fn crossing_order(&self) -> &[String] {
        &self.cross_order
    }
}
fn numeric_error(e: NumericError, path: &str) -> ModelError {
    ModelError::new("expression_numeric", path, e.to_string())
}
fn as_bool(v: &TypedValue, path: &str) -> Result<bool> {
    match v.value() {
        ScalarValue::Bool(b) => Ok(*b),
        _ => Err(error("expression_type", path, "expected Boolean value")),
    }
}
fn nonready(
    a: &Evaluation<TypedValue>,
    b: &Evaluation<TypedValue>,
) -> Option<Evaluation<TypedValue>> {
    if matches!(a, Evaluation::Missing) || matches!(b, Evaluation::Missing) {
        Some(Evaluation::Missing)
    } else if matches!(a, Evaluation::Warmup) || matches!(b, Evaluation::Warmup) {
        Some(Evaluation::Warmup)
    } else {
        None
    }
}
fn compare(a: &TypedValue, b: &TypedValue, path: &str) -> Result<std::cmp::Ordering> {
    a.numeric_coefficient()
        .and_then(|a| b.numeric_coefficient().map(|b| a.cmp(&b)))
        .map_err(|_| error("expression_type", path, "expected numeric operands"))
}
fn binary_value(op: BinaryOp, a: TypedValue, b: TypedValue, path: &str) -> Result<TypedValue> {
    let t = operation_type(op, a.descriptor(), b.descriptor(), path)?;
    let value = match op {
        BinaryOp::Equal => ScalarValue::Bool(a == b),
        BinaryOp::NotEqual => ScalarValue::Bool(a != b),
        BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
            let c = compare(&a, &b, path)?;
            ScalarValue::Bool(match op {
                BinaryOp::Less => c.is_lt(),
                BinaryOp::LessEqual => !c.is_gt(),
                BinaryOp::Greater => c.is_gt(),
                _ => !c.is_lt(),
            })
        }
        BinaryOp::And | BinaryOp::Or => ScalarValue::Bool(match op {
            BinaryOp::And => as_bool(&a, path)? && as_bool(&b, path)?,
            _ => as_bool(&a, path)? || as_bool(&b, path)?,
        }),
        _ => match (a.value(), b.value()) {
            (ScalarValue::U64(a), ScalarValue::U64(b)) => {
                let v = match op {
                    BinaryOp::Add => a.checked_add(*b),
                    BinaryOp::Subtract => a.checked_sub(*b),
                    BinaryOp::Multiply(_) => a.checked_mul(*b),
                    BinaryOp::Divide(_) => {
                        if *b == 0 {
                            return Err(numeric_error(NumericError::DivisionByZero, path));
                        }
                        if a % b != 0 {
                            return Err(numeric_error(NumericError::Inexact, path));
                        }
                        a.checked_div(*b)
                    }
                    _ => None,
                }
                .ok_or_else(|| numeric_error(NumericError::Overflow, path))?;
                ScalarValue::U64(v)
            }
            (ScalarValue::I64(a), ScalarValue::I64(b)) => {
                let v = match op {
                    BinaryOp::Add => a.checked_add(*b),
                    BinaryOp::Subtract => a.checked_sub(*b),
                    BinaryOp::Multiply(_) => a.checked_mul(*b),
                    BinaryOp::Divide(_) => {
                        if *b == 0 {
                            return Err(numeric_error(NumericError::DivisionByZero, path));
                        }
                        let q = a
                            .checked_div(*b)
                            .ok_or_else(|| numeric_error(NumericError::Overflow, path))?;
                        if a.checked_rem(*b) != Some(0) {
                            return Err(numeric_error(NumericError::Inexact, path));
                        }
                        Some(q)
                    }
                    _ => None,
                }
                .ok_or_else(|| numeric_error(NumericError::Overflow, path))?;
                ScalarValue::I64(v)
            }
            (ScalarValue::Decimal(a), ScalarValue::Decimal(b)) => ScalarValue::Decimal(
                match op {
                    BinaryOp::Add => a.checked_add(*b),
                    BinaryOp::Subtract => a.checked_sub(*b),
                    BinaryOp::Multiply(r) => a.checked_mul(*b, r),
                    BinaryOp::Divide(r) => a.checked_div(*b, r),
                    _ => {
                        return Err(error(
                            "expression_type",
                            path,
                            "unsupported decimal operator",
                        ));
                    }
                }
                .map_err(|e| numeric_error(e, path))?,
            ),
            _ => {
                return Err(error(
                    "expression_type",
                    path,
                    "unsupported arithmetic type",
                ));
            }
        },
    };
    TypedValue::new(t, value).map_err(|_| {
        error(
            "expression_type",
            path,
            "computed value differs from inferred output type",
        )
    })
}
struct Evaluator<'a> {
    inputs: &'a BTreeMap<String, Evaluation<TypedValue>>,
    crosses: BTreeMap<String, Evaluation<TypedValue>>,
    context: CallContext,
}
impl Evaluator<'_> {
    fn value(&mut self, e: &Expr, path: &str) -> Result<Evaluation<TypedValue>> {
        match e {
            Expr::Literal(v) => Ok(Evaluation::Ready(v.clone())),
            Expr::Input(k) => {
                let v = self
                    .inputs
                    .get(k)
                    .ok_or_else(|| error("expression_reference", path, "input unavailable"))?;
                if let Evaluation::Error(n) = v {
                    return Err(numeric_error(*n, path));
                }
                Ok(v.clone())
            }
            Expr::Cross(k) => self.crosses.get(k).cloned().ok_or_else(|| {
                error(
                    "expression_reference",
                    path,
                    "crossing subscription not advanced",
                )
            }),
            Expr::Not(e) => Ok(match self.value(e, path)? {
                Evaluation::Ready(v) => Evaluation::Ready(boolean(!as_bool(&v, path)?)),
                v => v,
            }),
            Expr::Binary { op, left, right } => {
                let a = self.value(left, path)?;
                if matches!(op, BinaryOp::And | BinaryOp::Or)
                    && let Evaluation::Ready(v) = &a
                {
                    let v = as_bool(v, path)?;
                    if (*op == BinaryOp::And && !v) || (*op == BinaryOp::Or && v) {
                        return Ok(a);
                    }
                }
                let b = self.value(right, path)?;
                if matches!(op, BinaryOp::And | BinaryOp::Or)
                    && let Evaluation::Ready(v) = &b
                {
                    let v = as_bool(v, path)?;
                    if (*op == BinaryOp::And && !v) || (*op == BinaryOp::Or && v) {
                        return Ok(b);
                    }
                }
                if let Some(v) = nonready(&a, &b) {
                    return Ok(v);
                }
                let (Evaluation::Ready(a), Evaluation::Ready(b)) = (a, b) else {
                    return Err(error(
                        "expression_type",
                        path,
                        "unexpected evaluation state",
                    ));
                };
                Ok(Evaluation::Ready(binary_value(*op, a, b, path)?))
            }
            Expr::Call {
                call,
                inputs,
                output,
            } => {
                let mut values = Vec::new();
                for p in &call.descriptor().declaration().inputs {
                    let e = &inputs
                        .iter()
                        .find(|(k, _)| *k == p.name)
                        .ok_or_else(|| {
                            error("expression_reference", path, "call input unavailable")
                        })?
                        .1;
                    values.push((p.name.clone(), self.value(e, path)?));
                }
                let context = self.context.clone();
                self.context.call_sequence = self
                    .context
                    .call_sequence
                    .checked_add(1)
                    .ok_or_else(|| numeric_error(NumericError::Overflow, path))?;
                let result = call
                    .invoke(context, values)
                    .map_err(|e| ModelError::new(e.code, path, e.detail))?;
                result
                    .outputs()
                    .get(output)
                    .cloned()
                    .ok_or_else(|| error("expression_reference", path, "call output unavailable"))
            }
        }
    }
}
pub struct ExpressionInstance {
    plan: Arc<ExpressionPlan>,
    run: String,
    instance: String,
    history: BTreeMap<String, Option<(TypedValue, TypedValue)>>,
    last_sequence: Option<u64>,
    accepted_steps: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpressionResult {
    pub sequence: u64,
    pub outputs: BTreeMap<String, Evaluation<TypedValue>>,
}
impl ExpressionInstance {
    pub fn new(
        plan: Arc<ExpressionPlan>,
        run: impl Into<String>,
        instance: impl Into<String>,
    ) -> Result<Self> {
        let run = run.into();
        let instance = instance.into();
        identifier(&run, "$.run")?;
        identifier(&instance, "$.instance")?;
        Ok(Self {
            plan,
            run,
            instance,
            history: BTreeMap::new(),
            last_sequence: None,
            accepted_steps: 0,
        })
    }
    pub fn last_sequence(&self) -> Option<u64> {
        self.last_sequence
    }
    pub fn accepted_steps(&self) -> u64 {
        self.accepted_steps
    }
    pub fn evaluate(
        &mut self,
        sequence: u64,
        inputs: Vec<(String, Evaluation<TypedValue>)>,
    ) -> Result<ExpressionResult> {
        if sequence == 0 || self.last_sequence.is_some_and(|last| sequence <= last) {
            return Err(error(
                "expression_sequence",
                "$.sequence",
                "expected a new positive logical sequence",
            ));
        }
        if inputs.len() != self.plan.draft.inputs.len() {
            return Err(error(
                "expression_arity",
                "$.inputs",
                "input count differs from plan",
            ));
        }
        let mut map = BTreeMap::new();
        for (key, v) in inputs {
            let p = self
                .plan
                .draft
                .inputs
                .iter()
                .find(|p| p.name == key)
                .ok_or_else(|| error("expression_reference", "$.inputs", "unknown input"))?;
            if let Evaluation::Ready(v) = &v
                && v.descriptor() != &p.value_type
            {
                return Err(error(
                    "expression_type",
                    "$.inputs",
                    "input type/unit/scale differs from declaration",
                ));
            }
            if map.insert(key, v).is_some() {
                return Err(error("expression_duplicate", "$.inputs", "duplicate input"));
            }
        }
        let mut evaluator = Evaluator {
            inputs: &map,
            crosses: BTreeMap::new(),
            context: CallContext {
                run: self.run.clone(),
                instance: self.instance.clone(),
                event_sequence: sequence,
                call_sequence: 1,
            },
        };
        let mut staged = self.history.clone();
        for id in &self.plan.cross_order {
            let c = self
                .plan
                .draft
                .crossings
                .iter()
                .find(|c| c.id == *id)
                .ok_or_else(|| {
                    error(
                        "expression_reference",
                        "$.crossings",
                        "subscription unavailable",
                    )
                })?;
            let path = format!("$.crossings.{id}");
            let left = evaluator.value(&c.left, &path)?;
            let right = evaluator.value(&c.right, &path)?;
            let output = if let Some(v) = nonready(&left, &right) {
                staged.insert(id.clone(), None);
                v
            } else {
                let (Evaluation::Ready(left), Evaluation::Ready(right)) = (left, right) else {
                    return Err(error("expression_type", &path, "invalid crossing values"));
                };
                let now = compare(&left, &right, &path)?;
                let crossed = if let Some(Some((a, b))) = self.history.get(id) {
                    let previous = compare(a, b, &path)?;
                    match c.direction {
                        CrossDirection::Above => !previous.is_gt() && now.is_gt(),
                        CrossDirection::Below => !previous.is_lt() && now.is_lt(),
                    }
                } else {
                    false
                };
                staged.insert(id.clone(), Some((left, right)));
                Evaluation::Ready(boolean(crossed))
            };
            evaluator.crosses.insert(id.clone(), output);
        }
        let mut outputs = BTreeMap::new();
        for (name, e) in &self.plan.draft.outputs {
            outputs.insert(
                name.clone(),
                evaluator.value(e, &format!("$.outputs.{name}"))?,
            );
        }
        let steps = self
            .accepted_steps
            .checked_add(1)
            .ok_or_else(|| numeric_error(NumericError::Overflow, "$.steps"))?;
        self.history = staged;
        self.last_sequence = Some(sequence);
        self.accepted_steps = steps;
        Ok(ExpressionResult { sequence, outputs })
    }
}
