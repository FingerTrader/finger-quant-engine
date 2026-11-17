//! Account-free, bounded chart studies. All formulas and defaults are engine-owned.
use super::*;
use graph::Node;

type StudyDefinition = (
    &'static str,
    Vec<(&'static str, &'static str)>,
    Vec<&'static str>,
);
fn definitions(standard: bool, oscillators: bool) -> Vec<StudyDefinition> {
    vec![
        ("sma", vec![("period", "20")], vec!["value"]),
        ("ema", vec![("period", "20")], vec!["value"]),
        (
            "macd",
            if standard {
                super::macd::DEFAULTS.to_vec()
            } else {
                vec![("fast", "12"), ("slow", "26"), ("signal", "9")]
            },
            if standard {
                vec!["macd_line", "signal_line", "histogram"]
            } else {
                vec!["line", "signal", "histogram"]
            },
        ),
        ("rsi", vec![("period", "14")], vec!["value"]),
        (
            "boll",
            vec![("period", "20"), ("deviation", "2")],
            vec!["middle", "upper", "lower"],
        ),
        ("atr", vec![("period", "14")], vec!["value"]),
        (
            "highest",
            vec![("period", "20"), ("offset", "0")],
            vec!["value"],
        ),
        (
            "lowest",
            vec![("period", "20"), ("offset", "0")],
            vec!["value"],
        ),
        (
            "kdj",
            if oscillators {
                super::oscillators::defaults("kdj")
            } else {
                vec![("period", "9"), ("smooth_k", "3"), ("smooth_d", "3")]
            },
            vec!["k", "d", "j"],
        ),
        (
            if oscillators { "WILLR" } else { "wr" },
            if oscillators {
                super::oscillators::defaults("WILLR")
            } else {
                vec![("period", "14")]
            },
            if oscillators {
                vec!["williams_r"]
            } else {
                vec!["value"]
            },
        ),
        (
            "cci",
            if oscillators {
                super::oscillators::defaults("cci")
            } else {
                vec![("period", "14")]
            },
            if oscillators {
                vec!["cci"]
            } else {
                vec!["value"]
            },
        ),
        ("volume", vec![("period", "5")], vec!["value"]),
    ]
}
pub fn describe(p: &V) -> Result<V> {
    exact(p, &[])?;
    Ok(o([
        ("profile", s(super::oscillators::PROFILE)),
        ("professional_profile", s(super::professional::PROFILE)),
        ("max_bars", V::from(MAX_BARS)),
        ("max_studies", V::from(32)),
        (
            "definitions",
            V::Array(
                definitions(true, true)
                    .into_iter()
                    .chain(super::professional::KINDS.iter().map(|kind| {
                        (
                            *kind,
                            super::professional::defaults(kind),
                            super::professional::ports(kind),
                        )
                    }))
                    .map(|(kind, params, ports)| {
                        o([
                            ("kind", s(kind)),
                            (
                                "display_name",
                                s(match kind {
                                    "MA" => "Moving Average",
                                    "BOLL" => "Bollinger Bands",
                                    "ATR" => "Average True Range",
                                    "ADX" => "ADX / DMI",
                                    "VWAP" => "Volume Weighted Average Price",
                                    _ => kind,
                                }),
                            ),
                            ("category", s("technical")),
                            (
                                "default_pane",
                                s(if super::professional::kinds(kind) {
                                    super::professional::pane(kind)
                                } else if matches!(kind, "sma" | "ema" | "boll") {
                                    "overlay"
                                } else {
                                    "pane"
                                }),
                            ),
                            (
                                "parameter_schema",
                                if super::professional::kinds(kind) {
                                    super::professional::schema(kind)
                                } else {
                                    o(params.iter().map(|(k, v)| {
                                        (
                                            *k,
                                            o([
                                                ("default", s(v)),
                                                (
                                                    "type",
                                                    s(
                                                        if matches!(
                                                            *k,
                                                            "length"
                                                                | "di_length"
                                                                | "adx_smoothing"
                                                                | "bar_interval_seconds"
                                                        ) {
                                                            "positive_integer"
                                                        } else if *k == "stddev_multiplier" {
                                                            "positive_number"
                                                        } else {
                                                            "enum"
                                                        },
                                                    ),
                                                ),
                                            ]),
                                        )
                                    }))
                                },
                            ),
                            (
                                "style_schema",
                                o([
                                    ("line_colors", s("per_output")),
                                    ("line_width", s("0.5..4")),
                                    ("band_fill", s("BOLL")),
                                    ("reference_level", s("ADX:25")),
                                ]),
                            ),
                            (
                                "version",
                                s(if super::professional::kinds(kind) {
                                    "4"
                                } else if kind == "macd" {
                                    "2"
                                } else if super::oscillators::kinds(kind) {
                                    "3"
                                } else {
                                    "1"
                                }),
                            ),
                            ("parameters", o(params.into_iter().map(|(k, v)| (k, s(v))))),
                            (
                                "outputs",
                                V::Array(
                                    ports
                                        .into_iter()
                                        .map(|port| {
                                            o([
                                                ("port", s(port)),
                                                (
                                                    "render",
                                                    s(if kind == "macd" && port == "histogram" {
                                                        "histogram"
                                                    } else {
                                                        "line"
                                                    }),
                                                ),
                                            ])
                                        })
                                        .collect(),
                                ),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]))
}
pub fn calculate(p: &V) -> Result<V> {
    exact(p, &["profile", "bars", "studies"])?;
    let professional = p["profile"] == super::professional::PROFILE;
    let oscillators = professional || p["profile"] == super::oscillators::PROFILE;
    let standard = oscillators || p["profile"] == super::macd::PROFILE;
    if !standard && p["profile"] != super::chart_compat::PROFILE {
        return Err(failure("chart_profile", "profile"));
    }
    let decoded;
    let bars = if let Some(text) = p["bars"].as_str() {
        decoded = crate::codec::decode_chart_bars(text.as_bytes())?;
        decoded
            .as_array()
            .ok_or_else(|| failure("chart_bars", "bars"))?
    } else {
        arr(p, "bars")?
    };
    let studies = arr(p, "studies")?;
    if bars.len() > MAX_BARS || studies.len() > 32 {
        return Err(failure("chart_quota", "$"));
    }
    let mut times = vec![];
    let mut data = vec![];
    for b in bars {
        exact(b, &["time", "open", "high", "low", "close", "volume"])?;
        let t = strv(b, "time")?
            .parse::<u64>()
            .map_err(|_| failure("chart_time", "time"))?;
        if times.last().is_some_and(|previous| *previous >= t) {
            return Err(failure("chart_time_order", "time"));
        }
        times.push(t);
        let mut row = vec![];
        for key in ["open", "high", "low", "close", "volume"] {
            let text = strv(b, key)?;
            if standard
                && (key != "volume" || professional)
                && studies.iter().any(|s| {
                    s["kind"].as_str().is_some_and(super::professional::kinds)
                        || s["kind"] == "macd"
                        || (oscillators
                            && s["kind"].as_str().is_some_and(|k| {
                                super::oscillators::kinds(k) || super::professional::kinds(k)
                            }))
                })
            {
                // Exactly the same accepted plain-decimal input domain as live graphs.
                super::macd::chart_decimal(text)
                    .map_err(|_| failure("macd_input_decimal_precision_12", key))?;
            }
            let value = text
                .parse::<f64>()
                .map_err(|_| failure("chart_numeric", key))?;
            if !value.is_finite() || value.abs() > 1e18 {
                return Err(failure("chart_numeric", key));
            }
            if professional && key == "volume" && value < 0.0 {
                return Err(failure("indicator_volume_nonnegative", key));
            }
            row.push(value);
        }
        if oscillators
            && studies.iter().any(|s| {
                s["kind"]
                    .as_str()
                    .is_some_and(|k| super::oscillators::kinds(k) || super::professional::kinds(k))
            })
            && (row[1] < row[2]
                || row[0] < row[2]
                || row[0] > row[1]
                || row[3] < row[2]
                || row[3] > row[1])
        {
            return Err(failure("indicator_invalid_ohlc", "bars"));
        }
        data.push(row);
    }
    let mut ids = BTreeSet::new();
    let mut columns = vec![];
    let mut values = vec![vec![]; bars.len()];
    for study in studies {
        exact(study, &["id", "kind", "parameters"])?;
        let id = strv(study, "id")?;
        let kind = strv(study, "kind")?;
        if id.is_empty() || id.len() > 128 || !ids.insert(id) {
            return Err(failure("chart_id", id));
        }
        let (_, defaults, ports) = definitions(standard, oscillators)
            .into_iter()
            .chain(
                super::professional::KINDS
                    .iter()
                    .filter(|_| professional)
                    .map(|kind| {
                        (
                            *kind,
                            super::professional::defaults(kind),
                            super::professional::ports(kind),
                        )
                    }),
            )
            .find(|d| d.0 == kind)
            .ok_or_else(|| failure("chart_kind", kind))?;
        let params = study["parameters"]
            .as_object()
            .ok_or_else(|| failure("chart_parameters", id))?;
        if params.keys().any(|k| !defaults.iter().any(|d| d.0 == k)) {
            return Err(failure("chart_parameter_unknown", id));
        }
        let mut settings: BTreeMap<String, String> = BTreeMap::new();
        for (key, default) in defaults {
            let v = params
                .get(key)
                .map(|v| v.as_str().ok_or_else(|| failure("chart_parameter", key)))
                .transpose()?
                .unwrap_or(default);
            if super::professional::kinds(kind)
                || (kind == "macd" && standard)
                || (oscillators && super::oscillators::kinds(kind))
            {
                // All MACD/oscillator fields are validated together below, without coercion.
            } else if key == "deviation" {
                let n = v
                    .parse::<f64>()
                    .map_err(|_| failure("chart_parameter", key))?;
                if !n.is_finite() || n <= 0.0 || n > 100.0 {
                    return Err(failure("chart_parameter", key));
                }
            } else {
                let n = v
                    .parse::<u64>()
                    .map_err(|_| failure("chart_parameter", key))?;
                if n > 10000 || (n == 0 && key != "offset") {
                    return Err(failure("chart_parameter", key));
                }
            }
            settings.insert(key.into(), v.into());
        }
        if super::professional::kinds(kind) {
            super::professional::validate(kind, &settings)?;
        }
        if kind == "macd" && standard {
            super::macd::validate(&settings)?;
        }
        if oscillators && super::oscillators::kinds(kind) {
            super::oscillators::validate(kind, &settings)?;
        }
        if kind == "macd"
            && settings["fast"].parse::<u64>().unwrap() >= settings["slow"].parse::<u64>().unwrap()
        {
            return Err(failure("chart_parameter", "fast"));
        }
        if kind == "boll" && settings["period"].parse::<u64>().unwrap() == 1 {
            return Err(failure("chart_parameter", "period"));
        }
        let node = Node {
            id: id.into(),
            kind: kind.into(),
            settings: settings.clone(),
            literals: BTreeMap::new(),
            inputs: BTreeMap::new(),
            outputs: ports
                .iter()
                .map(|p| (p.to_string(), "number".into()))
                .collect(),
        };
        let samples = data
            .iter()
            .map(|x| match kind {
                kind if oscillators && super::oscillators::kinds(kind) => x[..4].to_vec(),
                "atr" | "kdj" | "wr" | "cci" => vec![x[1], x[2], x[3]],
                "macd" if standard => x[..4].to_vec(),
                "volume" => vec![x[4]],
                _ => vec![x[3]],
            })
            .collect::<Vec<_>>();
        let output = if super::professional::kinds(kind) {
            super::chart_compat::professional_calculate(&node, &data, &times)?
        } else {
            super::chart_compat::calculate(&node, &samples)?
        };
        for port in ports {
            columns.push(o([
                ("id", s(id)),
                ("kind", s(kind)),
                ("port", s(port)),
                (
                    "parameters",
                    o(settings.iter().map(|(k, v)| (k.clone(), s(v)))),
                ),
                (
                    "render",
                    s(if kind == "macd" && port == "histogram" {
                        "histogram"
                    } else {
                        "line"
                    }),
                ),
            ]));
            for (i, row) in output.iter().enumerate() {
                values[i].push(row[port].value.as_ref().map_or(V::Null, Atom::json));
            }
        }
    }
    Ok(o([
        ("profile", p["profile"].clone()),
        ("columns", V::Array(columns)),
        (
            "rows",
            V::Array(
                times
                    .into_iter()
                    .zip(values)
                    .map(|(t, v)| o([("time", s(t)), ("values", V::Array(v))]))
                    .collect(),
            ),
        ),
    ]))
}
