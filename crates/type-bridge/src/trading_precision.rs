use crate::{BpError, BpResult};
use rust_decimal::{Decimal, RoundingStrategy, prelude::FromPrimitive};

#[derive(Debug, Clone, Copy)]
pub struct TradingPrecision {
    pub base_limit: MinMaxStep,
    pub base_market: MinMaxStep,
    pub quote: MinMaxStep,
}

#[derive(Debug, Clone, Copy)]
pub struct MinMaxStep {
    pub min: Decimal,
    pub max: Decimal,
    pub step: Decimal,
}

impl TradingPrecision {
    pub fn new(base_limit: MinMaxStep, base_market: MinMaxStep, quote: MinMaxStep) -> Self {
        Self {
            base_limit,
            base_market,
            quote,
        }
    }
    pub(crate) fn to_valid_base_limit(
        self,
        base: f64,
        strategy: RoundingStrategy,
    ) -> BpResult<Decimal> {
        Self::to_valid_decimal(base, self.base_limit, strategy)
    }
    pub(crate) fn to_valid_base_market(
        self,
        base: f64,
        strategy: RoundingStrategy,
    ) -> BpResult<Decimal> {
        Self::to_valid_decimal(base, self.base_market, strategy)
    }
    pub(crate) fn to_valid_quote(
        self,
        quote: f64,
        strategy: RoundingStrategy,
    ) -> BpResult<Decimal> {
        Self::to_valid_decimal(quote, self.quote, strategy)
    }
    fn to_valid_decimal(
        value: f64,
        min_max_step: MinMaxStep,
        strategy: RoundingStrategy,
    ) -> BpResult<Decimal> {
        let MinMaxStep { min, max, step } = min_max_step;
        let value_as_decimal = Decimal::from_f64(value).ok_or_else(|| BpError::InvalidValue {
            name: "Trading value".into(),
            value: value.to_string(),
        })?;
        let rounded_value = if step == Decimal::ZERO {
            value_as_decimal
        } else {
            let step_count = value_as_decimal / step;
            step_count.round_dp_with_strategy(0, strategy) * step
        };
        Ok(rounded_value.clamp(min, max))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn precision() -> TradingPrecision {
        let limits = MinMaxStep {
            min: Decimal::ZERO,
            max: Decimal::from(100),
            step: Decimal::new(1, 2),
        };
        TradingPrecision::new(limits, limits, limits)
    }

    #[test]
    fn non_finite_values_return_an_error() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let result = precision().to_valid_quote(value, RoundingStrategy::MidpointNearestEven);
            assert!(matches!(result, Err(BpError::InvalidValue { .. })));
        }
    }

    #[test]
    fn finite_values_are_rounded_and_clamped() {
        let precision = precision();
        let result = precision
            .to_valid_quote(1.234, RoundingStrategy::MidpointNearestEven)
            .unwrap();
        assert_eq!(result, Decimal::new(123, 2));
        let result = precision
            .to_valid_quote(1_000.0, RoundingStrategy::MidpointNearestEven)
            .unwrap();
        assert_eq!(result, Decimal::from(100));
    }
}
