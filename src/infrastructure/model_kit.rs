//! Model kits for the Blender add-on (tools/blender/vindictus) and the panel's 3D viewer: a studio
//! model of the client with the files it needs, in the game's folders, as a stored zip
//! (`<model>.mdl`, `.vvd`, `.dx90.vtx`, `.dx80.vtx`, `.sw.vtx`, the `.vmt` materials and their
//! `.vtf` textures, `kit.json`). Same content as scripts/client/model_kit.py.
use super::client_files::{ClientFiles, ClientFilesError};
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
    if ok { Ok(path) } else { Err(KitError::InvalidPath) }
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
    let end = rest.iter().take(MAX_PATH + 1).position(|&b| b == 0).ok_or(KitError::Malformed)?;
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
            cstring(mdl, at.checked_add_signed(relative as isize).ok_or(KitError::Malformed)?)
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
        path.replace('\\', "/").to_ascii_lowercase().trim_matches('/')
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
            let Some(vmt) = files.read(&vmt_path)? else { continue };
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

/// The `normal` model of entry `key` in `section` of player_costume.txt.
pub fn costume_model(text: &[u8], section: &str, key: i64) -> Option<String> {
    let text = String::from_utf8_lossy(text);
    let header = format!("\n\"{section}\"");
    let start = text.find(&header)?;
    let next = text[start + header.len()..]
        .find("\n\"")
        .map_or(text.len(), |i| start + header.len() + i);
    let body = &text[start..next];
    let entry = format!("\n\t\"{key}\"");
    let at = body.find(&entry)?;
    let block = &body[at..];
    let block = &block[..block.find("\n\t}").unwrap_or(block.len())];
    let line = block.lines().find(|l| l.trim_start().starts_with("\"normal\""))?;
    let value = line.split('"').nth(3)?;
    model_path(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_paths_are_bounded_game_paths() {
        assert_eq!(model_path("Models\\Player\\Fiona\\X.mdl").ok().as_deref(), Some("models/player/fiona/x.mdl"));
        for bad in ["models/../x.mdl", "materials/x.vmt", "models/x.mdl;rm", "models//x.mdl"] {
            assert!(model_path(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn costume_entries_resolve_to_their_normal_model() {
        let text = b"// c\r\n\"base\"\r\n{\r\n\t\"16\"\r\n\t{\r\n\t\t\"normal\"\t\"models/b.mdl\"\r\n\t}\r\n}\r\n\"inner_armor\"\r\n{\r\n\t\"16\"\r\n\t{\r\n\t\t\"category\"\t\"female\"\r\n\t\t\"normal\"\t\"models/player/fiona/fiona_inner_fringe.mdl\"\r\n\t}\r\n}\r\n";
        assert_eq!(costume_model(text, "inner_armor", 16).as_deref(), Some("models/player/fiona/fiona_inner_fringe.mdl"));
        assert_eq!(costume_model(text, "inner_armor", 17), None);
        assert_eq!(costume_section("AVATAR_TUNIC"), Some("upper_body"));
    }

    #[test]
    fn stored_zip_has_one_entry_per_file() {
        let zipped = zip(&[("a.txt".into(), b"hello".to_vec()), ("b/c.bin".into(), vec![1, 2, 3])]);
        assert_eq!(&zipped[..4], b"PK\x03\x04");
        assert_eq!(&zipped[zipped.len() - 22..zipped.len() - 18], b"PK\x05\x06");
        assert_eq!(u16::from_le_bytes([zipped[zipped.len() - 12], zipped[zipped.len() - 11]]), 2);
    }
}
