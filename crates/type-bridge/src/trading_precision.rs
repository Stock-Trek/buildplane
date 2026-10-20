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
    pub(crate) fn to_valid_base_limit(self, base: f64, strategy: RoundingStrategy) -> Decimal {
        Self::to_valid_decimal(base, self.base_limit, strategy)
    }
    pub(crate) fn to_valid_base_market(self, base: f64, strategy: RoundingStrategy) -> Decimal {
        Self::to_valid_decimal(base, self.base_market, strategy)
    }
    pub(crate) fn to_valid_quote(self, quote: f64, strategy: RoundingStrategy) -> Decimal {
        Self::to_valid_decimal(quote, self.quote, strategy)
    }
    fn to_valid_decimal(
        value: f64,
        min_max_step: MinMaxStep,
        strategy: RoundingStrategy,
    ) -> Decimal {
        let MinMaxStep { min, max, step } = min_max_step;
        let value_as_decimal = Decimal::from_f64(value).expect("Invalid value");
        let rounded_value = if step == Decimal::ZERO {
            value_as_decimal
        } else {
            let step_count = value_as_decimal / step;
            step_count.round_dp_with_strategy(0, strategy) * step
        };
        rounded_value.clamp(min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn min_max_step(min: Decimal, max: Decimal, step: Decimal) -> MinMaxStep {
        MinMaxStep { min, max, step }
    }

    #[test]
    fn zero_step_returns_input_value() {
        let value = 123.25;
        let precision = min_max_step(Decimal::ONE, Decimal::from(1000), Decimal::ZERO);

        let result = TradingPrecision::to_valid_decimal(
            value,
            precision,
            RoundingStrategy::MidpointAwayFromZero,
        );

        assert_eq!(result, Decimal::new(12325, 2));
    }

    #[test]
    fn zero_step_still_clamps_to_bounds() {
        let precision = min_max_step(Decimal::from(10), Decimal::from(100), Decimal::ZERO);

        assert_eq!(
            TradingPrecision::to_valid_decimal(
                1.0,
                precision,
                RoundingStrategy::MidpointAwayFromZero
            ),
            Decimal::from(10)
        );
        assert_eq!(
            TradingPrecision::to_valid_decimal(
                1000.0,
                precision,
                RoundingStrategy::MidpointAwayFromZero
            ),
            Decimal::from(100)
        );
    }

    #[test]
    fn non_zero_step_rounds_to_step() {
        let precision = min_max_step(Decimal::ZERO, Decimal::from(1000), Decimal::new(5, 1));

        assert_eq!(
            TradingPrecision::to_valid_decimal(123.25, precision, RoundingStrategy::ToZero),
            Decimal::new(12300, 2)
        );
        assert_eq!(
            TradingPrecision::to_valid_decimal(123.25, precision, RoundingStrategy::AwayFromZero),
            Decimal::new(12350, 2)
        );
    }
}
