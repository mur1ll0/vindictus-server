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
pub mod character_creation_admission;
pub mod character_creation_core;
pub mod character_creation_effects;
pub mod character_creation_ledger;
pub mod character_name_check;
pub mod character_name_proof;

pub mod character_costumes;

pub mod item_state;

pub mod character_base_stats;
pub mod character_skills;
pub mod fiona_appearance;
pub mod fiona_character_state;
pub mod fiona_initializer_profile;
pub mod fiona_inventory_profile;
pub mod initial_inventory;
pub mod item_colors;
pub mod item_stats;

pub mod character_initialization_logs;
pub mod fiona_skill_grants;

pub mod item_journals;

pub mod item_event_logs;

pub mod fiona_inventory_delivery;

pub mod shared_item_state;

pub mod fiona_customization;

pub mod fiona_managers;

pub mod character_vocation;

pub mod character_status_effects;

pub mod user_captcha;

pub mod character_transfer_guard;
