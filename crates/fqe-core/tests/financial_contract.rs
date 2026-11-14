use fqe_core::{
    InstrumentId,
    finance::{
        self, FinancialError, FinancialInstrument, FinancialRounding as R, InstrumentDraft,
        PricePolicy,
    },
    numeric::Decimal,
    types::CurrencyCode,
    utc::{self, TimeError},
};
fn decimal(s: &str, scale: u8) -> Decimal {
    finance::parse_decimal(s, scale).unwrap()
}
fn instrument(policy: PricePolicy) -> FinancialInstrument {
    FinancialInstrument::new(InstrumentDraft {
        instrument: InstrumentId::new("DEMO").unwrap(),
        currency: CurrencyCode::new("USD").unwrap(),
        price_step: decimal("0.05", 2),
        quantity_step: decimal("0.001", 3),
        money_step: decimal("0.01", 2),
        minimum_quantity: decimal("0.002", 3),
        minimum_notional: decimal("0", 2),
        prices: policy,
    })
    .unwrap()
}
#[test]
fn fixed_decimal_sources_are_exact_and_never_use_float_or_implicit_rounding() {
    for (text, scale, coefficient) in [
        ("0", 2, 0),
        ("120.0000", 2, 12000),
        ("-1.25", 2, -125),
        ("0.000001", 6, 1),
        ("1000000000000", 12, 1_000_000_000_000_000_000_000_000),
    ] {
        let v = decimal(text, scale);
        assert_eq!(v.coefficient(), coefficient);
        assert_eq!(decimal(&finance::decimal_text(v), scale), v);
    }
    for text in [
        "", "01", "+1", "1e2", "1.", ".5", " 1", "-0", "-0.00", "NaN",
    ] {
        assert!(finance::parse_decimal(text, 2).is_err(), "{text}");
    }
    assert_eq!(
        finance::parse_decimal("1.001", 2),
        Err(FinancialError::Inexact)
    );
    assert_eq!(
        finance::parse_decimal("999999999999999999999999999999999999999", 0),
        Err(FinancialError::Overflow)
    );
}
#[test]
fn independent_signed_half_ties_and_floor_ceiling_align_to_actual_step_not_decimal_places() {
    for (text, even, away, floor, ceiling) in [
        ("1.025", "1.00", "1.05", "1.00", "1.05"),
        ("1.075", "1.10", "1.10", "1.05", "1.10"),
        ("-1.025", "-1.00", "-1.05", "-1.05", "-1.00"),
        ("-1.075", "-1.10", "-1.10", "-1.10", "-1.05"),
    ] {
        for (rounding, expected) in [
            (R::HalfEven, even),
            (R::HalfAwayFromZero, away),
            (R::Floor, floor),
            (R::Ceiling, ceiling),
        ] {
            assert_eq!(
                finance::align(decimal(text, 3), decimal("0.05", 2), rounding).unwrap(),
                decimal(expected, 2)
            );
        }
        assert_eq!(
            finance::align(decimal(text, 3), decimal("0.05", 2), R::Exact),
            Err(FinancialError::Inexact)
        );
    }
    assert_eq!(
        finance::align(
            Decimal::new(i128::MAX, 0).unwrap(),
            Decimal::new(2, 0).unwrap(),
            R::Ceiling
        ),
        Err(FinancialError::Overflow)
    );
}
#[test]
fn instrument_price_quantity_currency_steps_and_checked_notionals_are_explicit() {
    let cash = instrument(PricePolicy::NonNegative);
    assert!(cash.price(decimal("-0.05", 2)).is_err());
    assert!(cash.price(decimal("1.01", 2)).is_err());
    assert!(cash.quantity(decimal("0.001", 3), false).is_err());
    assert!(cash.quantity(decimal("0", 3), true).is_ok());
    assert_eq!(
        cash.notional(decimal("120", 2), decimal("2", 3), R::Exact)
            .unwrap(),
        decimal("240", 2)
    );
    assert_eq!(
        cash.notional(decimal("1.05", 2), decimal("0.1", 3), R::HalfAwayFromZero)
            .unwrap(),
        decimal("0.11", 2)
    );
    let signed = instrument(PricePolicy::Signed);
    assert_eq!(
        signed
            .notional(decimal("-1.05", 2), decimal("0.1", 3), R::HalfAwayFromZero)
            .unwrap(),
        decimal("-0.11", 2)
    );
    assert!(signed.price(decimal("1000000000000.05", 2)).is_err());
    assert!(signed.quantity(decimal("1000000.001", 3), false).is_err());
    let mut d = cash.declaration().clone();
    d.money_step = decimal("0", 2);
    assert_eq!(
        FinancialInstrument::new(d).err(),
        Some(FinancialError::Step)
    );
}
#[test]
fn gregorian_utc_and_microsecond_mapping_are_exact_at_leaps_epoch_and_full_width_limits() {
    for (text, expected) in [
        ("1970-01-01T00:00:00Z", 0),
        ("1970-01-01T00:00:00.000001Z", 1000),
        ("2000-02-29T00:00:00Z", 951782400000000000),
        ("2024-02-29T12:34:56.123456Z", 1709210096123456000),
    ] {
        assert_eq!(utc::parse_utc_ns(text).unwrap(), expected);
        assert_eq!(
            utc::micro_to_ns(utc::ns_to_micro_exact(expected).unwrap()).unwrap(),
            expected
        );
    }
    for text in [
        "1900-02-29T00:00:00Z",
        "2100-02-29T00:00:00Z",
        "2026-02-30T00:00:00Z",
        "2026-01-01T24:00:00Z",
        "2016-12-31T23:59:60Z",
        "2026-01-01T00:00:00+09:00",
        "2026-01-01T00:00:00.0000001Z",
    ] {
        assert!(utc::parse_utc_ns(text).is_err(), "{text}");
    }
    assert_eq!(
        utc::parse_utc_ns("2555-01-01T00:00:00Z"),
        Err(TimeError::Overflow)
    );
    assert_eq!(utc::micro_to_ns(u64::MAX), Err(TimeError::Overflow));
    assert_eq!(utc::ns_to_micro_exact(1), Err(TimeError::Precision));
}
