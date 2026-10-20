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
            step_count.round_dp_with_strategy(0, strategy)
        };
        (rounded_value * step).clamp(min, max)
    }
}
