use crate::trading_precision::TradingPrecision;
use rust_decimal::{Decimal, RoundingStrategy};
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
                limit_price: self.quote_quantity(limit_price),
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
                callback_rate_bps,
                direction,
            },
        }
    }
    fn base_quantity(&self, quantity: f64, is_limit: bool) -> Decimal {
        if is_limit {
            self.trading_precision
                .to_valid_base_limit(quantity, self.rounding.price)
        } else {
            self.trading_precision
                .to_valid_base_market(quantity, self.rounding.price)
        }
    }
    fn quote_quantity(&self, quantity: f64) -> Decimal {
        self.trading_precision
            .to_valid_quote(quantity, self.rounding.price)
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
