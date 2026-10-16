//! CostumeManager.LoadFromDB projection for a lobby summary (no avatar equipment overlay).
use crate::{
    domain::character_costume::{Costume, CostumeProfile, CostumeSet},
    protocol::character_list::{Costume as Info, MAX_DICTIONARY},
};

fn set<T>(entries: &mut Option<Vec<(i32, T)>>, key: i32, value: T) {
    if let Some(entries) = entries {
        if let Some((_, old)) = entries.iter_mut().find(|(k, _)| *k == key) {
            *old = value
        } else {
            entries.push((key, value));
        }
    }
}
fn empty() -> Info {
    Info {
        costume_type_info: Some(vec![]),
        color_info: Some(vec![]),
        avatar_info: Some(vec![]),
        avatar_hide_info: Some(vec![]),
        pollution_info: Some(vec![]),
        effect_info: Some(vec![]),
        decoration_info: Some(vec![]),
        decoration_color_info: Some(vec![]),
        body_shape_info: Some(vec![]),
        shineness: -1,
        height: -1,
        bust: -1,
        painting_pos_x: -1,
        painting_pos_y: -1,
        painting_rotation: -1,
        painting_size: -1,
        hide_head_costume: false,
        cafe_type: 0,
        is_return: false,
        vip_code: 0,
        body_painting_pos_x: -1,
        body_painting_pos_y: -1,
        body_painting_rotation: -1,
        body_painting_size: -1,
        body_painting_side: -1,
        body_painting_clip: -1,
        body_painting_mode: -1,
        town_effect: 0,
    }
}
fn apply(info: &mut Info, row: &Costume) {
    let key = i32::from(row.key);
    match row.kind {
        0 => {
            if let Some(v) = row.serial {
                set(&mut info.costume_type_info, key, v)
            }
            for (offset, color) in row.colors.iter().enumerate() {
                if let Some(v) = color {
                    set(&mut info.color_info, 3 * key + offset as i32, *v)
                }
            }
            if let Some(v) = row.gloss {
                set(&mut info.pollution_info, key, v)
            }
            set(&mut info.effect_info, key, row.effect.unwrap_or(-1));
            set(&mut info.avatar_info, key, false);
        }
        1 => {
            if let Some(v) = row.serial {
                set(&mut info.decoration_info, key, v)
            }
            for (offset, color) in row.colors.iter().enumerate() {
                if let Some(v) = color {
                    set(&mut info.decoration_color_info, 3 * key + offset as i32, *v)
                }
            }
        }
        2 => {
            if let Some(v) = row.serial {
                match row.key {
                    0 => info.shineness = v,
                    1 => info.height = v,
                    2 => info.bust = v,
                    3 => info.painting_pos_x = v,
                    4 => info.painting_pos_y = v,
                    5 => info.painting_rotation = v,
                    6 => info.painting_size = v,
                    7 => info.hide_head_costume = v == 1,
                    8 => info.body_painting_pos_x = v,
                    9 => info.body_painting_pos_y = v,
                    10 => info.body_painting_rotation = v,
                    11 => info.body_painting_size = v,
                    12 => info.body_painting_side = v,
                    13 => info.body_painting_clip = v,
                    14 => info.body_painting_mode = v,
                    15..=19 => set(&mut info.avatar_hide_info, key + 39, v),
                    _ => {}
                }
            }
        }
        3 => {
            if let Some(v) = row.serial {
                // The explicit source Int32 -> Single cast rounds before division on both measured JITs.
                let value = (v as f32) / 1000.0_f32;
                // Source removes then adds, reusing the deleted Dictionary slot on Framework 4.8.
                set(&mut info.body_shape_info, key, value.to_bits());
            }
        }
        _ => {}
    }
}
/// Source applies all base rows before all item rows; absent fields never erase an earlier value.
/// Source Single conversion/division matches both measured Framework process architectures.
pub fn project(rows: &CostumeSet) -> Result<Info, CostumeProfile> {
    let mut info = empty();
    for base in [true, false] {
        for row in rows.rows().filter(|r| r.is_base == base) {
            apply(&mut info, row)
        }
    }
    let lengths = [
        info.costume_type_info.as_ref().map_or(0, Vec::len),
        info.color_info.as_ref().map_or(0, Vec::len),
        info.avatar_info.as_ref().map_or(0, Vec::len),
        info.avatar_hide_info.as_ref().map_or(0, Vec::len),
        info.pollution_info.as_ref().map_or(0, Vec::len),
        info.effect_info.as_ref().map_or(0, Vec::len),
        info.decoration_info.as_ref().map_or(0, Vec::len),
        info.decoration_color_info.as_ref().map_or(0, Vec::len),
        info.body_shape_info.as_ref().map_or(0, Vec::len),
    ];
    if lengths.iter().any(|n| *n > MAX_DICTIONARY) {
        return Err(CostumeProfile);
    }
    Ok(info)
}
