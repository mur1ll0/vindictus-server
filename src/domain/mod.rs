//! Foundation account invariants, independent of legacy credential formats.
mod account;
pub use account::{Username, hash_password, validate_password};

pub mod legacy_user;

pub mod game_user;

pub mod legacy_user_data;
pub mod login_account_data;
pub mod login_effects;
pub mod login_policy;
pub mod passport;
pub mod user_login_return;

pub mod character_data;
pub mod dormant_item;
pub mod feature_policy;

pub mod dormant_rewards;
pub mod item_argument;
pub mod item_class_ex;
pub mod item_location;
pub mod queued_item;
