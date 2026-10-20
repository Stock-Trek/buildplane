use serde::{Deserialize, Serialize};

macro_rules! math_ops {
    ($name:ident, $raw_type:ty) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub $raw_type);

        impl $name {
            pub const ZERO: $name = $name(0);
            pub const ONE: $name = $name(1);
            pub const TWO: $name = $name(2);
            pub const TEN: $name = $name(10);
        }

        impl std::ops::Add for $name {
            type Output = $name;
            fn add(self, rhs: Self) -> Self::Output {
                $name(self.0.saturating_add(rhs.0))
            }
        }
        impl std::ops::AddAssign for $name {
            fn add_assign(&mut self, rhs: Self) {
                self.0 = self.0.saturating_add(rhs.0);
            }
        }
        impl std::ops::Sub for $name {
            type Output = $name;
            fn sub(self, rhs: Self) -> Self::Output {
                $name(self.0.saturating_sub(rhs.0))
            }
        }
        impl std::ops::SubAssign for $name {
            fn sub_assign(&mut self, rhs: Self) {
                self.0 = self.0.saturating_sub(rhs.0);
            }
        }
        impl std::ops::Mul<$raw_type> for $name {
            type Output = $name;
            fn mul(self, rhs: $raw_type) -> Self::Output {
                $name(self.0.saturating_mul(rhs))
            }
        }
        impl std::ops::MulAssign<$raw_type> for $name {
            fn mul_assign(&mut self, rhs: $raw_type) {
                self.0 = self.0.saturating_mul(rhs);
            }
        }
        impl std::ops::Div for $name {
            type Output = $raw_type;
            fn div(self, rhs: $name) -> Self::Output {
                self.0.saturating_div(rhs.0)
            }
        }
        impl std::ops::Div<$raw_type> for $name {
            type Output = $name;
            fn div(self, rhs: $raw_type) -> Self::Output {
                $name(self.0.saturating_div(rhs))
            }
        }
        impl std::ops::DivAssign<$raw_type> for $name {
            fn div_assign(&mut self, rhs: $raw_type) {
                self.0 = self.0.saturating_div(rhs);
            }
        }
        impl std::ops::Rem for $name {
            type Output = $raw_type;
            fn rem(self, rhs: $name) -> Self::Output {
                self.0 % rhs.0
            }
        }
        impl std::ops::Rem<$raw_type> for $name {
            type Output = $name;
            fn rem(self, rhs: $raw_type) -> Self::Output {
                $name(self.0 % rhs)
            }
        }
        impl std::ops::RemAssign<$raw_type> for $name {
            fn rem_assign(&mut self, rhs: $raw_type) {
                self.0 = self.0 % rhs
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

math_ops!(Seconds, i64);
math_ops!(Milliseconds, i64);
math_ops!(Microseconds, i64);
math_ops!(Nanoseconds, i64);
math_ops!(UsageCount, u64);
