//! Persistence adapters for operational accounts and the partial legacy game model.
pub mod postgres;

pub mod frontend;

pub mod game_users;

pub mod legacy_users;
pub mod limited_accounts;
pub mod login_account_effects;
pub mod login_policy;
pub mod user_export;

pub mod character_export;
pub mod characters;
pub mod dormant_items;
pub mod item_catalog;
pub mod login_rewards;
pub mod reward_policy;

pub mod user_login;

pub mod second_password;

pub mod character_lobby;

pub mod lobby_export;

pub mod home_cafe_bonus;

pub mod lab_clock;
pub mod lab_runtime_config;

pub mod lab_frontend;

pub mod character_reservations;

pub mod character_collections;
pub mod character_name_check;
pub mod character_name_proof;
