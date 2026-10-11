//! Persistence adapters for operational accounts and the partial legacy game model.
pub mod postgres;

pub mod frontend;

pub mod game_users;

pub mod legacy_users;
pub mod limited_accounts;
pub mod login_account_effects;
pub mod login_policy;
pub mod user_export;
