use crate::trading_precision::TradingPrecision;
use rust_decimal::{Decimal, RoundingStrategy, prelude::ToPrimitive};
use stock_trek_types::cex::{
    activation::Activation, asset_id::AssetId, order_request::OrderRequest,
    preferences::CexRoundingPreferences, quantity::Quantity, trigger_direction::TriggerDirection,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct FloatsToDecimals {
    trading_precision: TradingPrecision,
    rounding: CexRoundingPreferences,
}

impl FloatsToDecimals {
    pub fn new(trading_precision: TradingPrecision, rounding: CexRoundingPreferences) -> Self {
        Self {
            trading_precision,
            rounding,
        }
    }
    pub fn floats_to_decimals(
        &self,
        order_request: OrderRequest<AssetId, f64>,
    ) -> OrderRequest<AssetId, Decimal> {
        match order_request {
            OrderRequest::Limit {
                activation,
                base,
                limit_price,
                quantity,
                quote,
                side,
                tag,
                time_in_force,
            } => OrderRequest::Limit {
                base,
                quote,
                side,
                activation: self.activation(activation),
                limit_price: self.price(limit_price),
                time_in_force,
                quantity: self.quantity(quantity, true),
                tag,
            },
            OrderRequest::MarketBuy {
                base,
                quote,
                activation,
                quote_quantity,
                tag,
            } => OrderRequest::MarketBuy {
                base,
                quote,
                activation: self.activation(activation),
                quote_quantity: self.quote_quantity(quote_quantity),
                tag,
            },
            OrderRequest::MarketSell {
                base,
                quote,
                activation,
                base_quantity,
                tag,
            } => OrderRequest::MarketSell {
                base,
                quote,
                activation: self.activation(activation),
                base_quantity: self.base_quantity(base_quantity, false),
                tag,
            },
        }
    }

    fn activation(&self, activation: Activation<f64>) -> Activation<Decimal> {
        match activation {
            Activation::Immediate => Activation::Immediate,
            Activation::PriceTriggered {
                activation_price,
                basis,
                direction,
                mode,
            } => Activation::PriceTriggered {
                activation_price: self.trading_precision.to_valid_quote(
                    activation_price,
                    self.activation_price_rounding_strategy(direction),
                ),
                basis,
                direction,
                mode,
            },
            Activation::Trailing {
                activation_price,
                basis,
                callback_rate_bps,
                direction,
            } => Activation::Trailing {
                activation_price: self.trading_precision.to_valid_quote(
                    activation_price,
                    self.activation_price_rounding_strategy(direction),
                ),
                basis,
                callback_rate_bps: self.callback_rate_bps(callback_rate_bps),
                direction,
            },
        }
    }
    fn base_quantity(&self, quantity: f64, is_limit: bool) -> Decimal {
        if is_limit {
            self.trading_precision
                .to_valid_base_limit(quantity, self.rounding.quantity)
        } else {
            self.trading_precision
                .to_valid_base_market(quantity, self.rounding.quantity)
        }
    }
    fn quote_quantity(&self, quantity: f64) -> Decimal {
        self.trading_precision
            .to_valid_quote(quantity, self.rounding.quantity)
    }
    fn price(&self, price: f64) -> Decimal {
        self.trading_precision
            .to_valid_quote(price, self.rounding.price)
    }
    fn callback_rate_bps(&self, callback_rate_bps: u32) -> u32 {
        Decimal::from(callback_rate_bps)
            .round_dp_with_strategy(0, self.rounding.callback_rate_bps)
            .to_u32()
            .unwrap_or(callback_rate_bps)
    }
    fn quantity(&self, quantity: Quantity<f64>, is_limit: bool) -> Quantity<Decimal> {
        match quantity {
            Quantity::OfBase(q) => Quantity::OfBase(self.base_quantity(q, is_limit)),
            Quantity::OfQuote(q) => Quantity::OfQuote(self.quote_quantity(q)),
        }
    }
    fn activation_price_rounding_strategy(&self, direction: TriggerDirection) -> RoundingStrategy {
        match direction {
            TriggerDirection::Above => self.rounding.activation_price_triggered_above,
            TriggerDirection::Below => self.rounding.activation_price_triggered_below,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trading_precision::MinMaxStep;
    use stock_trek_types::cex::{
        price_basis::PriceBasis, side::Side, tag::Tag, time_in_force::TimeInForce,
        trigger_mode::TriggerMode,
    };

    fn precision() -> TradingPrecision {
        let base = MinMaxStep {
            min: Decimal::ZERO,
            max: Decimal::from(1_000),
            step: Decimal::new(1, 1),
        };
        let quote = MinMaxStep {
            min: Decimal::ZERO,
            max: Decimal::from(1_000),
            step: Decimal::ONE,
        };
        TradingPrecision::new(base, base, quote)
    }

    fn preferences() -> CexRoundingPreferences {
        CexRoundingPreferences {
            activation_price_triggered_above: RoundingStrategy::ToZero,
            activation_price_triggered_below: RoundingStrategy::AwayFromZero,
            price: RoundingStrategy::ToZero,
            quantity: RoundingStrategy::AwayFromZero,
            callback_rate_bps: RoundingStrategy::ToZero,
        }
    }

    fn converter() -> FloatsToDecimals {
        FloatsToDecimals::new(precision(), preferences())
    }

    #[test]
    fn applies_price_rounding_only_to_prices() {
        let converted = converter().floats_to_decimals(OrderRequest::Limit {
            base: AssetId::Bitcoin,
            quote: AssetId::TetherUSD,
            side: Side::Buy,
            activation: Activation::PriceTriggered {
                activation_price: 123.4,
                basis: PriceBasis::Last,
                direction: TriggerDirection::Above,
                mode: TriggerMode::Touch,
            },
            limit_price: 123.9,
            time_in_force: TimeInForce::GoodTillCancelled,
            quantity: Quantity::OfBase(2.34),
            tag: Tag::new("tag"),
        });

        let OrderRequest::Limit {
            limit_price,
            quantity,
            activation,
            ..
        } = converted
        else {
            panic!("expected limit order");
        };
        assert_eq!(limit_price, Decimal::from(123));
        let Quantity::OfBase(quantity) = quantity else {
            panic!("expected base quantity");
        };
        assert_eq!(quantity, Decimal::new(24, 1));
        let Activation::PriceTriggered {
            activation_price,
            basis,
            direction,
            mode,
        } = activation
        else {
            panic!("expected price-triggered activation");
        };
        assert_eq!(activation_price, Decimal::from(123));
        assert_eq!(basis, PriceBasis::Last);
        assert_eq!(direction, TriggerDirection::Above);
        assert_eq!(mode, TriggerMode::Touch);
    }

    #[test]
    fn applies_quantity_rounding_to_quote_and_market_quantities() {
        let converter = converter();

        let market_buy = converter.floats_to_decimals(OrderRequest::MarketBuy {
            base: AssetId::Bitcoin,
            quote: AssetId::TetherUSD,
            activation: Activation::Immediate,
            quote_quantity: 100.2,
            tag: Tag::new("tag"),
        });
        let OrderRequest::MarketBuy { quote_quantity, .. } = market_buy else {
            panic!("expected market buy order");
        };
        assert_eq!(quote_quantity, Decimal::from(101));

        let market_sell = converter.floats_to_decimals(OrderRequest::MarketSell {
            base: AssetId::Bitcoin,
            quote: AssetId::TetherUSD,
            activation: Activation::Immediate,
            base_quantity: 2.34,
            tag: Tag::new("tag"),
        });
        let OrderRequest::MarketSell { base_quantity, .. } = market_sell else {
            panic!("expected market sell order");
        };
        assert_eq!(base_quantity, Decimal::new(24, 1));
    }

    #[test]
    fn applies_activation_price_and_callback_rate_rounding() {
        let converted = converter().floats_to_decimals(OrderRequest::MarketSell {
            base: AssetId::Bitcoin,
            quote: AssetId::TetherUSD,
            activation: Activation::Trailing {
                activation_price: 123.4,
                basis: PriceBasis::Last,
                callback_rate_bps: 50,
                direction: TriggerDirection::Below,
            },
            base_quantity: 2.34,
            tag: Tag::new("tag"),
        });

        let OrderRequest::MarketSell { activation, .. } = converted else {
            panic!("expected market sell order");
        };
        let Activation::Trailing {
            activation_price,
            basis,
            callback_rate_bps,
            direction,
        } = activation
        else {
            panic!("expected trailing activation");
        };
        assert_eq!(activation_price, Decimal::from(124));
        assert_eq!(basis, PriceBasis::Last);
        assert_eq!(callback_rate_bps, 50);
        assert_eq!(direction, TriggerDirection::Below);
    }
}
