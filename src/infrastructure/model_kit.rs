//! Model kits for the Blender add-on (tools/blender/vindictus) and the panel's 3D viewer: a studio
//! model of the client with the files it needs, in the game's folders, as a stored zip
//! (`<model>.mdl`, `.vvd`, `.dx90.vtx`, `.dx80.vtx`, `.sw.vtx`, the `.vmt` materials and their
//! `.vtf` textures, `kit.json`). Same content as scripts/client/model_kit.py.
use super::client_files::{ClientFiles, ClientFilesError};
use std::collections::BTreeSet;
use thiserror::Error;

const MODEL_SUFFIXES: [(&str, bool); 5] = [
    (".mdl", true),
    (".vvd", true),
    (".dx90.vtx", true),
    (".dx80.vtx", false),
    (".sw.vtx", false),
];
const MAX_TEXTURES: usize = 64;
const MAX_PATH: usize = 200;
/// The texture parameters of the client's materials (Heroes_* shaders included).
const TEXTURE_PARAMS: &str = r#"(?i)"\$(basetexture|bumpmap|basetexture2|detail|envmapmask|phongexponenttexture|lightwarptexture|blendmodulatetexture|normalmap|iris|corneatexture|ambientoccltexture|selfillummask|tintmasktexture)"\s+"([^"]*)""#;

#[derive(Debug, Error)]
pub enum KitError {
    #[error("model path is invalid")]
    InvalidPath,
    #[error("model file is missing")]
    Missing,
    #[error("studio model is malformed")]
    Malformed,
    #[error(transparent)]
    Client(#[from] ClientFilesError),
}

/// Lower case `models/....mdl` of letters, digits, `_`, `-`, `.` and `/`, without `..`.
pub fn model_path(path: &str) -> Result<String, KitError> {
    let path = path.trim().replace('\\', "/").to_ascii_lowercase();
    let ok = path.len() <= MAX_PATH
        && path.starts_with("models/")
        && path.ends_with(".mdl")
        && !path.contains("..")
        && !path.contains("//")
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'/'));
    if ok {
        Ok(path)
    } else {
        Err(KitError::InvalidPath)
    }
}

fn i32_at(data: &[u8], at: usize) -> Result<i32, KitError> {
    data.get(at..at + 4)
        .and_then(|b| b.try_into().ok())
        .map(i32::from_le_bytes)
        .ok_or(KitError::Malformed)
}

fn offset(value: i32) -> Result<usize, KitError> {
    usize::try_from(value).map_err(|_| KitError::Malformed)
}

fn cstring(data: &[u8], at: usize) -> Result<String, KitError> {
    let rest = data.get(at..).ok_or(KitError::Malformed)?;
    let end = rest
        .iter()
        .take(MAX_PATH + 1)
        .position(|&b| b == 0)
        .ok_or(KitError::Malformed)?;
    Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
}

/// Texture names and material folders of a studio model (studiohdr_t fields at 204 and 212).
pub fn model_materials(mdl: &[u8]) -> Result<(Vec<String>, Vec<String>), KitError> {
    if mdl.get(..4) != Some(b"IDST") {
        return Err(KitError::Malformed);
    }
    let (num_textures, texture_index) = (i32_at(mdl, 204)?, offset(i32_at(mdl, 208)?)?);
    let (num_cd, cd_index) = (i32_at(mdl, 212)?, offset(i32_at(mdl, 216)?)?);
    let (num_textures, num_cd) = (offset(num_textures)?, offset(num_cd)?);
    if num_textures > MAX_TEXTURES || num_cd > MAX_TEXTURES {
        return Err(KitError::Malformed);
    }
    let textures = (0..num_textures)
        .map(|i| {
            let at = texture_index + 64 * i;
            let relative = i32_at(mdl, at)?;
            cstring(
                mdl,
                at.checked_add_signed(relative as isize)
                    .ok_or(KitError::Malformed)?,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let folders = (0..num_cd)
        .map(|i| cstring(mdl, offset(i32_at(mdl, cd_index + 4 * i)?)?))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((textures, folders))
}

fn game_path(prefix: &str, path: &str, suffix: &str) -> String {
    format!(
        "{prefix}{}{suffix}",
        path.replace('\\', "/")
            .to_ascii_lowercase()
            .trim_matches('/')
    )
}

/// The kit files (game path, bytes) of a model, `kit.json` last.
pub fn build(files: &ClientFiles, model: &str) -> Result<Vec<(String, Vec<u8>)>, KitError> {
    let model = model_path(model)?;
    let base = &model[..model.len() - 4];
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for (suffix, required) in MODEL_SUFFIXES {
        let path = format!("{base}{suffix}");
        match files.read(&path)? {
            Some(data) => out.push((path, data)),
            None if required => return Err(KitError::Missing),
            None => {}
        }
    }
    let (textures, folders) = model_materials(&out[0].1)?;
    let params = regex::Regex::new(TEXTURE_PARAMS).map_err(|_| KitError::Malformed)?;
    for texture in &textures {
        let mut found = false;
        for folder in &folders {
            let vmt_path = game_path("materials/", &format!("{folder}{texture}"), ".vmt");
            let Some(vmt) = files.read(&vmt_path)? else {
                continue;
            };
            let text = String::from_utf8_lossy(&vmt).into_owned();
            out.push((vmt_path, vmt));
            for capture in params.captures_iter(&text) {
                let name = &capture[2];
                if name.is_empty() {
                    continue;
                }
                let vtf_path = game_path("materials/", name, ".vtf");
                if out.iter().any(|(p, _)| *p == vtf_path) {
                    continue;
                }
                match files.read(&vtf_path)? {
                    Some(vtf) => out.push((vtf_path, vtf)),
                    None => missing.push(vtf_path),
                }
            }
            found = true;
            break;
        }
        if !found {
            missing.push(texture.clone());
        }
    }
    let mut names: Vec<&str> = out.iter().map(|(p, _)| p.as_str()).collect();
    names.sort_unstable();
    let manifest = serde_json::json!({
        "format_version": 1, "model": model, "files": names, "missing": missing,
    });
    out.push(("kit.json".into(), manifest.to_string().into_bytes()));
    Ok(out)
}

/// A stored (uncompressed) zip of the files: the textures are already compressed or small.
pub fn zip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let crc = crc32fast::hash(data);
        let local = out.len() as u32;
        let size = data.len() as u32;
        let name_len = name.len() as u16;
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0x21, 0]);
        for v in [crc, size, size] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0x21, 0]);
        for v in [crc, size, size] {
            central.extend_from_slice(&v.to_le_bytes());
        }
        central.extend_from_slice(&name_len.to_le_bytes());
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&local.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let start = out.len() as u32;
    let count = files.len() as u16;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&[0; 2]);
    out
}

/// player_costume.txt section of an item category (ItemClassInfo.Category).
pub fn costume_section(category: &str) -> Option<&'static str> {
    Some(match category.trim_start_matches("AVATAR_") {
        "INNERARMOR" => "inner_armor",
        "TUNIC" => "upper_body",
        "PANTS" => "lower_body",
        "GLOVES" => "hand",
        "BOOTS" => "foot",
        "HELM" => "head",
        _ => return None,
    })
}

/// Where the client finds the model of an item: player_costume.txt for clothes and armor,
/// scripts/weapon_<EquipClass>.txt (`WeaponData` → `SubTypeInfoOverride`, `playermodel`) for
/// weapons and scripts/shields.txt (`Fiona`, `normal`) for shields and large shields. The entry
/// number is EquipItemInfo.CostumeType in all of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelScript {
    Costume(&'static str),
    Weapon(String),
    Shield,
}

impl ModelScript {
    /// The script of an item from its Category and EquipClass.
    pub fn of(category: &str, equip_class: Option<&str>) -> Option<Self> {
        if let Some(section) = costume_section(category) {
            return Some(Self::Costume(section));
        }
        let class = equip_class?.to_ascii_lowercase();
        match class.as_str() {
            "shield" | "largeshield" => Some(Self::Shield),
            _ if category == "WEAPON"
                && (1..=32).contains(&class.len())
                && class.bytes().all(|b| b.is_ascii_lowercase()) =>
            {
                Some(Self::Weapon(class))
            }
            _ => None,
        }
    }
    /// The name the item editor writes in an added item's costume entry (`section`).
    pub fn section(&self) -> String {
        match self {
            Self::Costume(section) => (*section).to_owned(),
            Self::Weapon(class) => format!("weapon_{class}"),
            Self::Shield => "shields".to_owned(),
        }
    }
    pub fn file(&self) -> String {
        match self {
            Self::Costume(_) => "scripts/player_costume.txt".to_owned(),
            Self::Weapon(class) => format!("scripts/weapon_{class}.txt"),
            Self::Shield => "scripts/shields.txt".to_owned(),
        }
    }
    /// Clothes and armor take CostumeType numbers from the editor's own range; weapon and shield
    /// numbers continue their script's numbering.
    pub fn costume_range(&self) -> bool {
        matches!(self, Self::Costume(_))
    }
    fn path(&self) -> Vec<&str> {
        match self {
            Self::Costume(section) => vec![section],
            Self::Weapon(_) => vec!["WeaponData", "SubTypeInfoOverride"],
            Self::Shield => vec!["Fiona"],
        }
    }
    fn field(&self) -> &'static str {
        match self {
            Self::Weapon(_) => "playermodel",
            _ => "normal",
        }
    }
    /// Entry numbers of the script's block.
    pub fn keys(&self, text: &[u8]) -> BTreeSet<i64> {
        self.entries(text)
            .into_iter()
            .filter_map(|(k, _)| k.parse().ok())
            .collect()
    }
    /// The model of entry `key`.
    pub fn model(&self, text: &[u8], key: i64) -> Option<String> {
        let value = self.value(text, key, self.field())?;
        model_path(&value).ok()
    }
    /// A field of entry `key` (for example the body `category` of a costume).
    pub fn value(&self, text: &[u8], key: i64, field: &str) -> Option<String> {
        let wanted = key.to_string();
        self.entries(text)
            .into_iter()
            .find(|(k, _)| *k == wanted)?
            .1
            .into_iter()
            .find(|(f, _)| f.eq_ignore_ascii_case(field))
            .map(|(_, v)| v)
    }
    fn entries(&self, text: &[u8]) -> Vec<(String, Vec<(String, String)>)> {
        let text = String::from_utf8_lossy(text);
        let tree = keyvalues(&text);
        let mut node = &tree;
        for key in self.path() {
            let child = node.iter().find_map(|(k, v)| match v {
                KeyValue::Block(child) if k.eq_ignore_ascii_case(key) => Some(child),
                _ => None,
            });
            let Some(child) = child else {
                return Vec::new();
            };
            node = child;
        }
        node.iter()
            .filter_map(|(k, v)| match v {
                KeyValue::Block(fields) => Some((
                    k.clone(),
                    fields
                        .iter()
                        .filter_map(|(f, v)| match v {
                            KeyValue::Text(t) => Some((f.clone(), t.clone())),
                            KeyValue::Block(_) => None,
                        })
                        .collect(),
                )),
                KeyValue::Text(_) => None,
            })
            .collect()
    }
}

/// A KeyValues node: text or a block of pairs.
#[derive(Debug, PartialEq, Eq)]
enum KeyValue {
    Text(String),
    Block(Vec<(String, KeyValue)>),
}

const MAX_KV_DEPTH: usize = 32;

/// Valve KeyValues as the client scripts write them: quoted or bare tokens, braces, `//`
/// comments and `[$WIN32]` conditions (ignored). Malformed trailing input is dropped.
fn keyvalues(text: &str) -> Vec<(String, KeyValue)> {
    let mut tokens: Vec<String> = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        match c {
            c if c.is_whitespace() => {}
            '/' if text[at..].starts_with("//") => {
                while chars.next_if(|(_, c)| *c != '\n').is_some() {}
            }
            '{' | '}' => tokens.push(c.to_string()),
            '"' => {
                let mut token = String::from("\"");
                for (_, c) in chars.by_ref() {
                    if c == '"' {
                        break;
                    }
                    token.push(c);
                }
                tokens.push(token);
            }
            _ => {
                let mut token = c.to_string();
                while let Some((_, c)) =
                    chars.next_if(|(_, c)| !c.is_whitespace() && !matches!(c, '{' | '}' | '"'))
                {
                    token.push(c);
                }
                tokens.push(token);
            }
        }
    }
    let mut at = 0;
    kv_block(&tokens, &mut at, 0)
}

fn kv_block(tokens: &[String], at: &mut usize, depth: usize) -> Vec<(String, KeyValue)> {
    let mut pairs = Vec::new();
    while let Some(token) = tokens.get(*at) {
        *at += 1;
        if token == "}" {
            break;
        }
        if token == "{" || token.starts_with('[') {
            continue;
        }
        let key = token.trim_start_matches('"').to_owned();
        match tokens.get(*at).map(String::as_str) {
            Some("{") => {
                *at += 1;
                let child = if depth < MAX_KV_DEPTH {
                    kv_block(tokens, at, depth + 1)
                } else {
                    Vec::new()
                };
                pairs.push((key, KeyValue::Block(child)));
            }
            Some("}") | None => {}
            Some(value) => {
                *at += 1;
                pairs.push((
                    key,
                    KeyValue::Text(value.trim_start_matches('"').to_owned()),
                ));
            }
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_paths_are_bounded_game_paths() {
        assert_eq!(
            model_path("Models\\Player\\Fiona\\X.mdl").ok().as_deref(),
            Some("models/player/fiona/x.mdl")
        );
        for bad in [
            "models/../x.mdl",
            "materials/x.vmt",
            "models/x.mdl;rm",
            "models//x.mdl",
        ] {
            assert!(model_path(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn costume_entries_resolve_to_their_normal_model() {
        let text = b"// c\r\n\"base\"\r\n{\r\n\t\"16\"\r\n\t{\r\n\t\t\"normal\"\t\"models/b.mdl\"\r\n\t}\r\n}\r\n\"inner_armor\"\r\n{\r\n\t\"16\"\r\n\t{\r\n\t\t\"category\"\t\"female\"\r\n\t\t\"normal\"\t\"models/player/fiona/fiona_inner_fringe.mdl\"\r\n\t}\r\n}\r\n";
        assert_eq!(
            ModelScript::Costume("inner_armor")
                .model(text, 16)
                .as_deref(),
            Some("models/player/fiona/fiona_inner_fringe.mdl")
        );
        assert_eq!(ModelScript::Costume("inner_armor").model(text, 17), None);
        assert_eq!(costume_section("AVATAR_TUNIC"), Some("upper_body"));
    }

    #[test]
    fn weapon_and_shield_entries_resolve_through_their_scripts() {
        let weapon = b"// LongSword\r\nWeaponData\r\n{\r\n\t\"playermodel\"\t\"models/weapon/longsword/gladius.mdl\"\r\n\t\"SubTypeInfoOverride\"\r\n\t{\r\n\t\t// comment { not a block\r\n\t\t\"1\"\r\n\t\t{\r\n\t\t\t\"viewmodel\"\t\"models/weapon/longsword/longsword_swordwind.mdl\"\r\n\t\t\t\"playermodel\"\t\"models/weapon/longsword/longsword_swordwind.mdl\"\r\n\t\t}\r\n\t\t// \"5\" removed\r\n\t\t\"6\" { \"playermodel\" \"models/weapon/longsword/x.mdl\" [$WIN32] }\r\n\t}\r\n}\r\n";
        let script = ModelScript::of("WEAPON", Some("LONGSWORD")).unwrap();
        assert_eq!(script.file(), "scripts/weapon_longsword.txt");
        assert_eq!(script.keys(weapon), BTreeSet::from([1, 6]));
        assert_eq!(
            script.model(weapon, 1).as_deref(),
            Some("models/weapon/longsword/longsword_swordwind.mdl")
        );
        let shields = b"\"Fiona\"\r\n{\r\n\t\"normal_paladin_subtype_id\"\t\"23\"\r\n\t\"15\"\r\n\t{\r\n\t\t\"normal\"\t\"models/weapon/largeshield/largeshield_kite.mdl\"\r\n\t}\r\n}\r\n";
        let shield = ModelScript::of("LARGESHIELD", Some("LARGESHIELD")).unwrap();
        assert_eq!(
            (shield.section(), shield.keys(shields)),
            ("shields".to_owned(), BTreeSet::from([15]))
        );
        assert_eq!(
            shield.model(shields, 15).as_deref(),
            Some("models/weapon/largeshield/largeshield_kite.mdl")
        );
        let inner = ModelScript::of("INNERARMOR", Some("CUSTOMIZE")).unwrap();
        assert_eq!(inner, ModelScript::Costume("inner_armor"));
        assert_eq!(ModelScript::of("HAIR", Some("CUSTOMIZE")), None);
        assert_eq!(ModelScript::of("NECKLACE", Some("Necklace")), None);
    }

    #[test]
    fn stored_zip_has_one_entry_per_file() {
        let zipped = zip(&[
            ("a.txt".into(), b"hello".to_vec()),
            ("b/c.bin".into(), vec![1, 2, 3]),
        ]);
        assert_eq!(&zipped[..4], b"PK\x03\x04");
        assert_eq!(&zipped[zipped.len() - 22..zipped.len() - 18], b"PK\x05\x06");
        assert_eq!(
            u16::from_le_bytes([zipped[zipped.len() - 12], zipped[zipped.len() - 11]]),
            2
        );
    }
}
