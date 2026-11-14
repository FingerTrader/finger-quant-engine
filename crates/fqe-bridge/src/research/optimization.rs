//! Training-only bounded grid. Validation is evaluated once after selection.
use super::*;
struct Grid {
    variants: Vec<BTreeMap<String, String>>,
    training: V,
    validation: V,
    objective: String,
    maximize: bool,
}
const MAX_TRIALS: usize = 100;
const MAX_TOTAL_BARS: usize = 200_000;
pub fn validate(input: &V) -> Result<V> {
    let prepared = prepare_grid(input)?;
    Ok(o([
        ("valid", V::Bool(true)),
        ("profile", s(PROFILE)),
        (
            "execution_id",
            s(json::content_id(
                b"FQE-RESEARCH-OPTIMIZER/1\n",
                &json::encode(input, crate::MAX_REQUEST)?,
            )),
        ),
        ("registry_id", s(crate::bridge_id())),
        ("trials", s(prepared.variants.len())),
    ]))
}
fn prepare_grid(input: &V) -> Result<Grid> {
    exact(input, &["kind", "training", "validation"])?;
    if input["kind"] != "research_optimization" {
        return Err(failure("research_optimization_kind", "$"));
    }
    let train = input["training"].clone();
    let validation = input["validation"].clone();
    let a = Prepared::new(&train)?;
    let b = Prepared::new(&validation)?;
    let train_start = a.streams[a.main].selection.performance_start_ns();
    let train_end = a.streams[a.main]
        .selection
        .bars()
        .last()
        .expect("validated")
        .input()
        .close_ns;
    let valid_start = b.streams[b.main].selection.performance_start_ns();
    if valid_start < train_end || valid_start <= train_start {
        return Err(failure(
            "research_validation_must_follow_training",
            "$.validation",
        ));
    }
    let ids = a
        .streams
        .iter()
        .map(|s| s.selection.dataset().bars_id().to_string())
        .collect::<BTreeSet<_>>();
    if b.streams
        .iter()
        .any(|s| ids.contains(&s.selection.dataset().bars_id().to_string()))
    {
        return Err(failure("research_train_validation_overlap", "$"));
    }
    let mut ma = json::decode_input(strv(&train, "model")?.as_bytes())?;
    let mut mb = json::decode_input(strv(&validation, "model")?.as_bytes())?;
    for m in [&mut ma, &mut mb] {
        m["context"]["dataset"] = s("");
        for n in m["nodes"].as_array_mut().expect("validated") {
            if n["kind"] == "bars" {
                n["settings"]["dataset"] = s("");
            }
        }
    }
    if ma != mb {
        return Err(failure("research_validation_strategy_differs", "$"));
    }
    let context = &a.graph.context;
    if context.get("searchMethod").map(String::as_str) != Some("grid") {
        return Err(failure("research_search_method", "$"));
    }
    let budget = ctx_uint(context, "maxTrials", MAX_TRIALS as u64)? as usize;
    if budget == 0 {
        return Err(failure("research_trial_budget", "$"));
    }
    let objective = context
        .get("objective")
        .ok_or_else(|| failure("research_objective", "$"))?
        .clone();
    if !["total_return", "max_drawdown_ratio"].contains(&objective.as_str()) {
        return Err(failure("research_objective", "$"));
    }
    let direction = context.get("direction").map(String::as_str).unwrap_or("");
    if !["maximize", "minimize"].contains(&direction) {
        return Err(failure("research_direction", "$"));
    }
    let mut variants = vec![BTreeMap::new()];
    let mut dimensions = 0;
    for p in arr(&ma, "parameters")?
        .iter()
        .filter(|p| p["optimize"] == true)
    {
        dimensions += 1;
        if dimensions > 4 {
            return Err(failure("research_dimension_quota", "$"));
        }
        let mut choices = vec![];
        if p["type"] == "bool" {
            choices = vec!["false".into(), "true".into()];
        } else {
            let lo = num(strv(p, "minimum")?)?;
            let hi = num(strv(p, "maximum")?)?;
            let step = num(strv(p, "step")?)?;
            if lo.coefficient() > hi.coefficient() || step.coefficient() <= 0 {
                return Err(failure("research_grid_range", "$"));
            }
            let mut value = lo;
            while value.coefficient() <= hi.coefficient() {
                if choices.len() >= 32 {
                    return Err(failure("research_grid_choices", "$"));
                }
                let text = if p["type"] == "integer" {
                    if value.coefficient() % FACTOR != 0 {
                        return Err(failure("research_integer_grid", "$"));
                    }
                    (value.coefficient() / FACTOR).to_string()
                } else {
                    decimal_text(value)
                };
                choices.push(text);
                value = add(value, step)?;
            }
        }
        let mut expanded = vec![];
        for variant in &variants {
            for value in &choices {
                let mut variant = variant.clone();
                variant.insert(strv(p, "id")?.into(), value.clone());
                expanded.push(variant);
                if expanded.len() > budget {
                    return Err(failure("research_grid_exceeds_budget", "$"));
                }
            }
        }
        variants = expanded;
    }
    if dimensions == 0 {
        return Err(failure("research_no_optimized_parameters", "$"));
    }
    let training_bars = a
        .streams
        .iter()
        .map(|s| s.selection.bars().len())
        .sum::<usize>();
    let validation_bars = b
        .streams
        .iter()
        .map(|s| s.selection.bars().len())
        .sum::<usize>();
    if training_bars
        .checked_mul(variants.len())
        .and_then(|n| n.checked_add(validation_bars))
        .is_none_or(|n| n > MAX_TOTAL_BARS)
    {
        return Err(failure("research_optimization_source_budget", "$"));
    }
    Ok(Grid {
        variants,
        training: train,
        validation,
        objective,
        maximize: direction == "maximize",
    })
}
fn variant(input: &V, values: &BTreeMap<String, String>) -> Result<V> {
    let mut input = input.clone();
    let mut model = json::decode_input(strv(&input, "model")?.as_bytes())?;
    for parameter in model["parameters"].as_array_mut().expect("validated") {
        if let Some(value) = values.get(strv(parameter, "id")?) {
            parameter["value"] = s(value);
        }
    }
    input["model"] = s(String::from_utf8(json::encode(&model, crate::MAX_REQUEST)?)
        .map_err(|_| failure("research_model", "$"))?);
    Ok(input)
}
pub fn run(input: &V, cancel: &AtomicBool) -> Result<Output> {
    let Grid {
        variants,
        training,
        validation,
        objective,
        maximize,
    } = prepare_grid(input)?;
    let mut trials = vec![];
    let mut winner: Option<(usize, Decimal, Vec<u8>)> = None;
    let mut partial = None;
    let mut processed = 0;
    for (i, values) in variants.iter().enumerate() {
        if cancel.load(Ordering::Acquire) {
            break;
        }
        let result = Prepared::new(&variant(&training, values)?)?.run(cancel)?;
        processed += result.processed;
        let report: V = serde_json::from_slice(&result.bytes)
            .map_err(|_| failure("research_native_report", "$"))?;
        if result.cancelled {
            partial = Some(result.bytes);
            break;
        }
        let score = num(strv(&report["metrics"], &objective)?)?;
        trials.push(o([
            ("trial", s(i + 1)),
            (
                "parameters",
                o(values.iter().map(|(k, v)| (k.as_str(), s(v)))),
            ),
            ("score", ds(score)),
            ("metrics", report["metrics"].clone()),
            ("result_id", s(&result.result_id)),
            ("manifest_id", s(&result.manifest_id)),
        ]));
        if winner.as_ref().is_none_or(|(_, best, _)| {
            if maximize {
                score.coefficient() > best.coefficient()
            } else {
                score.coefficient() < best.coefficient()
            }
        }) {
            winner = Some((i, score, result.bytes));
        }
    }
    let cancelled = cancel.load(Ordering::Acquire);
    let (winner_index, mut report) = if let Some((index, _, bytes)) = winner {
        (
            Some(index),
            serde_json::from_slice::<V>(&bytes)
                .map_err(|_| failure("research_native_report", "$"))?,
        )
    } else {
        let bytes = match partial {
            Some(b) => b,
            None => Prepared::new(&training)?.run(cancel)?.bytes,
        };
        (
            None,
            serde_json::from_slice::<V>(&bytes)
                .map_err(|_| failure("research_native_report", "$"))?,
        )
    };
    let validation_report = if !cancelled {
        let index = winner_index.ok_or_else(|| failure("research_no_winner", "$"))?;
        let result = Prepared::new(&variant(&validation, &variants[index])?)?.run(cancel)?;
        processed += result.processed;
        serde_json::from_slice::<V>(&result.bytes)
            .map_err(|_| failure("research_native_report", "$"))?
    } else {
        V::Null
    };
    let cancelled = cancel.load(Ordering::Acquire);
    report["terminal"] = s(if cancelled { "cancelled" } else { "completed" });
    report["manifest"]["optimization_input_id"] = s(json::content_id(
        b"FQE-RESEARCH-OPTIMIZER/1\n",
        &json::encode(input, crate::MAX_REQUEST)?,
    ));
    report["optimization"] = o([
        ("selection_basis", s("training-only;stable-first-tie")),
        ("objective", s(objective)),
        (
            "direction",
            s(if maximize { "maximize" } else { "minimize" }),
        ),
        ("completed", V::Bool(!cancelled)),
        ("trials", V::Array(trials)),
        (
            "winner_parameters",
            if cancelled {
                V::Null
            } else {
                winner_index.map_or(V::Null, |i| {
                    o(variants[i].iter().map(|(k, v)| (k.as_str(), s(v))))
                })
            },
        ),
        ("validation", validation_report),
    ]);
    report["manifest_id"] = s(json::content_id(
        b"FQE-RESEARCH-MANIFEST/1\n",
        &json::encode(&report["manifest"], crate::MAX_REQUEST)?,
    ));
    let bytes = crate::codec::encode(&report, crate::MAX_RESPONSE - 1024)?;
    Ok(Output {
        result_id: json::content_id(b"FQE-BACKTEST-RESULT/1\n", &bytes).to_string(),
        manifest_id: report["manifest_id"]
            .as_str()
            .expect("native manifest")
            .into(),
        bytes,
        processed,
        cancelled,
    })
}
