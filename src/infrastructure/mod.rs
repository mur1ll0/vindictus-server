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
pub mod item_economy;
pub mod item_enhance;
pub mod login_rewards;
pub mod npc_scripts;
pub mod reward_policy;

pub mod user_login;

pub mod second_password;

pub mod character_lobby;

pub mod lobby_export;

pub mod home_cafe_bonus;

pub mod lab_clock;
pub mod lab_runtime_config;

pub mod lab_frontend;
pub mod lab_peer;

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

pub mod newbie_attendance;

pub mod fiona_title_preset;

pub mod item_loader_state;

pub mod fiona_item_bootstrap;

pub mod mail_reference;

pub mod channel_server;
pub mod minimum_assets;
pub mod minimum_character;
pub mod minimum_character_list;
pub mod minimum_selection;

pub mod minimum_inventory;

pub mod equipment_profile;
pub mod minimum_enchant;
pub mod minimum_enhance;
pub mod minimum_equipment;
pub mod minimum_manufacture;
pub mod minimum_skill_enhance;

pub mod minimum_ap;
pub mod minimum_costume;
pub mod minimum_equip;
pub mod minimum_give;
pub mod minimum_growth;
pub mod minimum_item_logs;
mod minimum_item_stat;
pub mod minimum_mission;
pub mod minimum_mission_finish;
pub mod minimum_move;
pub mod minimum_quest;
pub mod minimum_quickslot;
pub mod minimum_repair;
pub mod minimum_restore;
pub mod minimum_shop;
pub mod minimum_skills;
pub mod minimum_story;
pub mod minimum_use;
pub mod relay;
pub mod skill_profile;
