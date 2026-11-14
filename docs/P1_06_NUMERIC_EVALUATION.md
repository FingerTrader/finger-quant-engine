# P1-06 Numeric Evaluation

Generated deliberately by scripts/generate_indicator_fixtures.py, Python stdlib Fraction; no engine import or binary invocation.

Exact unrounded rational recurrences versus candidate quantized working scales; both report HalfEven scale-12 outputs. Inputs are the tracked exact scale-12 coefficients. Maximum absolute errors over the nine datasets (563 samples):

| Output | work12 error | work18 error |
| --- | --- | --- |
| ema2 | 4.99813851369e-13 | 4.99813851369e-13 |
| ema3 | 9.29475873375e-13 | 5e-13 |
| line | 1.28488471415e-12 | 4.95270110042e-13 |
| signal | 1.53246291284e-12 | 4.99296824028e-13 |
| histogram | 7.48508959813e-13 | 4.98760458742e-13 |
| rsi2 | 1.0678338666e-11 | 4.97215969935e-13 |

Selection: work18/output12, explicit HalfEven at each seed, recurrence and output; preserves six guard digits. Work12 increases accumulated/RSI error. Binary64 cannot distinguish one 1e-12 output increment near 1e12; see public Rust boundary tests. No new decimal dependency is needed in this bounded slice.

These measurements cover periods 2/3 and the listed datasets, not every financial series. Contract error bounds, supported range and RSI conditioning guard are in INDICATOR_CONTRACT.md. Public Rust tests separately compare all outputs to the tracked exact-rational rounded coefficients within one output coefficient (1e-12), and assert readiness and strict crossing decisions. The candidate experiment is not a claim that Rust was executed by this script.
