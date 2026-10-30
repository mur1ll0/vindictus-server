//! Recovered filled CharacterListMessage/CharacterSummary/CostumeInfo with its PetStatusInfo.
//! Does not create characters or manufacture a list from missing persisted state.
pub use super::empty_character_list::{CATEGORY, TYPE_ID};
use super::{
    encode_integer,
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};
pub const MAX_CHARACTERS: usize = 1000;
pub const MAX_DICTIONARY: usize = 128;
pub type Dictionary<T> = Option<Vec<(i32, T)>>;
#[derive(PartialEq, Eq)]
pub struct Message<'a> {
    pub characters: Option<Vec<Option<Summary<'a>>>>,
    pub max_free: i32,
    pub max_purchased: i32,
    pub max_premium: i32,
    pub prologue_played: u8,
    pub preset_used: i32,
    pub party_states: Option<Vec<i32>>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Costume {
    pub costume_type_info: Dictionary<i32>,
    pub color_info: Dictionary<i32>,
    pub avatar_info: Dictionary<bool>,
    pub avatar_hide_info: Dictionary<i32>,
    pub pollution_info: Dictionary<u8>,
    pub effect_info: Dictionary<i32>,
    pub decoration_info: Dictionary<i32>,
    pub decoration_color_info: Dictionary<i32>,
    pub shineness: i32,
    pub height: i32,
    pub bust: i32,
    pub painting_pos_x: i32,
    pub painting_pos_y: i32,
    pub painting_rotation: i32,
    pub painting_size: i32,
    pub hide_head_costume: bool,
    pub cafe_type: i32,
    pub is_return: bool,
    pub vip_code: i32,
    pub body_painting_pos_x: i32,
    pub body_painting_pos_y: i32,
    pub body_painting_rotation: i32,
    pub body_painting_size: i32,
    pub body_painting_side: i32,
    pub body_painting_clip: i32,
    pub body_painting_mode: i32,
    pub body_shape_info: Dictionary<u32>,
    pub town_effect: i32,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Summary<'a> {
    pub cid: i64,
    pub nexon_sn: i32,
    pub character_id: Option<&'a str>,
    pub character_sn: i32,
    pub base_character: u8,
    pub level: i32,
    pub exp: i32,
    pub level_up_exp: i32,
    pub title: i32,
    pub title_count: i32,
    pub costume: Option<Costume>,
    pub daily_micro_play_count: i32,
    pub daily_free_play_count: i32,
    pub quote: Option<&'a str>,
    pub total_used_ap: i32,
    pub guild_id: i32,
    pub guild_name: Option<&'a str>,
    pub cafe_type: i32,
    pub is_premium: bool,
    pub has_bonus_effect: bool,
    pub is_return: bool,
    pub is_deleting: bool,
    pub delete_wait_left_sec: i32,
    pub is_should_name_change: bool,
    pub vip_code: i32,
    pub vocation_class: i32,
    pub vocation_level: i32,
    pub vocation_exp: i32,
    pub vocation_level_up_exp: i32,
    pub vocation_skill_point_available: i32,
    pub free_match_win_count: i32,
    pub free_match_lose_count: i32,
    /// The current pet (PetManager.CurrentPetStatusInfo); QueryCharacters supplies null.
    pub pet: Option<super::pet::PetStatusInfo>,
    pub is_event_jumping: bool,
    pub free_title_name: Option<&'a str>,
    pub pattern: i32,
    pub arena_win_count: i32,
    pub arena_lose_count: i32,
    pub arena_successive_win_count: i32,
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CharacterListError {
    #[error("duplicate costume dictionary key")]
    DuplicateKey,
    #[error("unsupported filled character-list type/category")]
    Type,
    #[error(transparent)]
    Payload(#[from] PayloadError),
}
fn count(w: &mut Writer, n: Option<usize>, limit: usize) -> Result<(), CharacterListError> {
    match n {
        None => w.append(&[0])?,
        Some(n) if n <= limit => w.append(&encode_integer((n + 1) as i32))?,
        Some(_) => return Err(PayloadError::TooLarge.into()),
    }
    Ok(())
}
fn bounded(r: &mut Reader<'_>, limit: usize) -> Result<Option<usize>, CharacterListError> {
    let n = r.count()?;
    if n.is_some_and(|n| n > limit) {
        return Err(PayloadError::TooLarge.into());
    }
    Ok(n)
}
trait Scalar: Sized {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError>;
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError>;
}
impl Scalar for i32 {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        w.append(&self.to_be_bytes())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        Ok(Self::from_be_bytes(r.fixed()?))
    }
}
impl Scalar for i64 {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        w.append(&self.to_be_bytes())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        Ok(Self::from_be_bytes(r.fixed()?))
    }
}
impl Scalar for u32 {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        w.append(&self.to_be_bytes())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        Ok(Self::from_be_bytes(r.fixed()?))
    }
}
impl Scalar for u8 {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        w.append(&[*self])
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        r.byte()
    }
}
impl Scalar for bool {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        w.append(&[u8::from(*self)])
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        Ok(r.byte()? != 0)
    }
}
fn write_dictionary<T: Scalar>(
    w: &mut Writer,
    d: &Dictionary<T>,
) -> Result<(), CharacterListError> {
    count(w, d.as_ref().map(Vec::len), MAX_DICTIONARY)?;
    if let Some(d) = d {
        for (i, (k, v)) in d.iter().enumerate() {
            if d[..i].iter().any(|(other, _)| other == k) {
                return Err(CharacterListError::DuplicateKey);
            }
            k.write(w)?;
            v.write(w)?;
        }
    }
    Ok(())
}
fn read_dictionary<T: Scalar>(r: &mut Reader<'_>) -> Result<Dictionary<T>, CharacterListError> {
    let Some(n) = bounded(r, MAX_DICTIONARY)? else {
        return Ok(None);
    };
    let mut rows = Vec::with_capacity(n);
    for _ in 0..n {
        let k = i32::read(r)?;
        if rows.iter().any(|(other, _)| *other == k) {
            return Err(CharacterListError::DuplicateKey);
        }
        rows.push((k, T::read(r)?));
    }
    Ok(Some(rows))
}
pub(super) fn write_costume(w: &mut Writer, c: Option<&Costume>) -> Result<(), CharacterListError> {
    let Some(c) = c else {
        w.append(&[0])?;
        return Ok(());
    };
    w.append(&[1])?;
    write_dictionary(w, &c.costume_type_info)?;
    write_dictionary(w, &c.color_info)?;
    write_dictionary(w, &c.avatar_info)?;
    write_dictionary(w, &c.avatar_hide_info)?;
    write_dictionary(w, &c.pollution_info)?;
    write_dictionary(w, &c.effect_info)?;
    write_dictionary(w, &c.decoration_info)?;
    write_dictionary(w, &c.decoration_color_info)?;
    c.shineness.write(w)?;
    c.height.write(w)?;
    c.bust.write(w)?;
    c.painting_pos_x.write(w)?;
    c.painting_pos_y.write(w)?;
    c.painting_rotation.write(w)?;
    c.painting_size.write(w)?;
    c.hide_head_costume.write(w)?;
    c.cafe_type.write(w)?;
    c.is_return.write(w)?;
    c.vip_code.write(w)?;
    c.body_painting_pos_x.write(w)?;
    c.body_painting_pos_y.write(w)?;
    c.body_painting_rotation.write(w)?;
    c.body_painting_size.write(w)?;
    c.body_painting_side.write(w)?;
    c.body_painting_clip.write(w)?;
    c.body_painting_mode.write(w)?;
    write_dictionary(w, &c.body_shape_info)?;
    c.town_effect.write(w)?;
    Ok(())
}
pub(super) fn read_costume(r: &mut Reader<'_>) -> Result<Option<Costume>, CharacterListError> {
    if r.byte()? == 0 {
        return Ok(None);
    }
    Ok(Some(Costume {
        costume_type_info: read_dictionary(r)?,
        color_info: read_dictionary(r)?,
        avatar_info: read_dictionary(r)?,
        avatar_hide_info: read_dictionary(r)?,
        pollution_info: read_dictionary(r)?,
        effect_info: read_dictionary(r)?,
        decoration_info: read_dictionary(r)?,
        decoration_color_info: read_dictionary(r)?,
        shineness: i32::read(r)?,
        height: i32::read(r)?,
        bust: i32::read(r)?,
        painting_pos_x: i32::read(r)?,
        painting_pos_y: i32::read(r)?,
        painting_rotation: i32::read(r)?,
        painting_size: i32::read(r)?,
        hide_head_costume: bool::read(r)?,
        cafe_type: i32::read(r)?,
        is_return: bool::read(r)?,
        vip_code: i32::read(r)?,
        body_painting_pos_x: i32::read(r)?,
        body_painting_pos_y: i32::read(r)?,
        body_painting_rotation: i32::read(r)?,
        body_painting_size: i32::read(r)?,
        body_painting_side: i32::read(r)?,
        body_painting_clip: i32::read(r)?,
        body_painting_mode: i32::read(r)?,
        body_shape_info: read_dictionary(r)?,
        town_effect: i32::read(r)?,
    }))
}
pub(super) fn write_summary(
    w: &mut Writer,
    s: Option<&Summary<'_>>,
) -> Result<(), CharacterListError> {
    let Some(s) = s else {
        w.append(&[0])?;
        return Ok(());
    };
    w.append(&[1])?;
    s.cid.write(w)?;
    s.nexon_sn.write(w)?;
    w.nullable(s.character_id.map(str::as_bytes))?;
    s.character_sn.write(w)?;
    s.base_character.write(w)?;
    s.level.write(w)?;
    s.exp.write(w)?;
    s.level_up_exp.write(w)?;
    s.title.write(w)?;
    s.title_count.write(w)?;
    write_costume(w, s.costume.as_ref())?;
    s.daily_micro_play_count.write(w)?;
    s.daily_free_play_count.write(w)?;
    w.nullable(s.quote.map(str::as_bytes))?;
    s.total_used_ap.write(w)?;
    s.guild_id.write(w)?;
    w.nullable(s.guild_name.map(str::as_bytes))?;
    s.cafe_type.write(w)?;
    s.is_premium.write(w)?;
    s.has_bonus_effect.write(w)?;
    s.is_return.write(w)?;
    s.is_deleting.write(w)?;
    s.delete_wait_left_sec.write(w)?;
    s.is_should_name_change.write(w)?;
    s.vip_code.write(w)?;
    s.vocation_class.write(w)?;
    s.vocation_level.write(w)?;
    s.vocation_exp.write(w)?;
    s.vocation_level_up_exp.write(w)?;
    s.vocation_skill_point_available.write(w)?;
    s.free_match_win_count.write(w)?;
    s.free_match_lose_count.write(w)?;
    super::pet::write_pet(w, s.pet.as_ref())?;
    s.is_event_jumping.write(w)?;
    w.nullable(s.free_title_name.map(str::as_bytes))?;
    s.pattern.write(w)?;
    s.arena_win_count.write(w)?;
    s.arena_lose_count.write(w)?;
    s.arena_successive_win_count.write(w)?;
    Ok(())
}
pub(super) fn read_summary<'a>(
    r: &mut Reader<'a>,
) -> Result<Option<Summary<'a>>, CharacterListError> {
    if r.byte()? == 0 {
        return Ok(None);
    }
    let cid = i64::read(r)?;
    let nexon_sn = i32::read(r)?;
    let character_id = r.string()?;
    let character_sn = i32::read(r)?;
    let base_character = u8::read(r)?;
    let level = i32::read(r)?;
    let exp = i32::read(r)?;
    let level_up_exp = i32::read(r)?;
    let title = i32::read(r)?;
    let title_count = i32::read(r)?;
    let costume = read_costume(r)?;
    let daily_micro_play_count = i32::read(r)?;
    let daily_free_play_count = i32::read(r)?;
    let quote = r.string()?;
    let total_used_ap = i32::read(r)?;
    let guild_id = i32::read(r)?;
    let guild_name = r.string()?;
    let cafe_type = i32::read(r)?;
    let is_premium = bool::read(r)?;
    let has_bonus_effect = bool::read(r)?;
    let is_return = bool::read(r)?;
    let is_deleting = bool::read(r)?;
    let delete_wait_left_sec = i32::read(r)?;
    let is_should_name_change = bool::read(r)?;
    let vip_code = i32::read(r)?;
    let vocation_class = i32::read(r)?;
    let vocation_level = i32::read(r)?;
    let vocation_exp = i32::read(r)?;
    let vocation_level_up_exp = i32::read(r)?;
    let vocation_skill_point_available = i32::read(r)?;
    let free_match_win_count = i32::read(r)?;
    let free_match_lose_count = i32::read(r)?;
    let pet = super::pet::read_pet(r)?;
    let is_event_jumping = bool::read(r)?;
    let free_title_name = r.string()?;
    let pattern = i32::read(r)?;
    let arena_win_count = i32::read(r)?;
    let arena_lose_count = i32::read(r)?;
    let arena_successive_win_count = i32::read(r)?;
    Ok(Some(Summary {
        cid,
        nexon_sn,
        character_id,
        character_sn,
        base_character,
        level,
        exp,
        level_up_exp,
        title,
        title_count,
        costume,
        daily_micro_play_count,
        daily_free_play_count,
        quote,
        total_used_ap,
        guild_id,
        guild_name,
        cafe_type,
        is_premium,
        has_bonus_effect,
        is_return,
        is_deleting,
        delete_wait_left_sec,
        is_should_name_change,
        vip_code,
        vocation_class,
        vocation_level,
        vocation_exp,
        vocation_level_up_exp,
        vocation_skill_point_available,
        free_match_win_count,
        free_match_lose_count,
        pet,
        is_event_jumping,
        free_title_name,
        pattern,
        arena_win_count,
        arena_lose_count,
        arena_successive_win_count,
    }))
}
pub fn encode(
    message: Option<&Message<'_>>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, CharacterListError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), CharacterListError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        w.append(&[1])?;
        count(&mut w, m.characters.as_ref().map(Vec::len), MAX_CHARACTERS)?;
        if let Some(chars) = &m.characters {
            for c in chars {
                write_summary(&mut w, c.as_ref())?;
            }
        }
        m.max_free.write(&mut w)?;
        m.max_purchased.write(&mut w)?;
        m.max_premium.write(&mut w)?;
        m.prologue_played.write(&mut w)?;
        m.preset_used.write(&mut w)?;
        count(&mut w, m.party_states.as_ref().map(Vec::len), 1000)?;
        if let Some(states) = &m.party_states {
            for s in states {
                s.write(&mut w)?;
            }
        }
        Ok(())
    })();
    if result.is_err() {
        w.bytes.zeroize();
    }
    result?;
    Ok(Zeroizing::new(w.bytes))
}
pub fn decode(body: &[u8], max_body: usize) -> Result<Option<Message<'_>>, CharacterListError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        let characters = if let Some(n) = bounded(&mut r, MAX_CHARACTERS)? {
            let mut chars = Vec::with_capacity(n);
            for _ in 0..n {
                chars.push(read_summary(&mut r)?)
            }
            Some(chars)
        } else {
            None
        };
        let max_free = i32::read(&mut r)?;
        let max_purchased = i32::read(&mut r)?;
        let max_premium = i32::read(&mut r)?;
        let prologue_played = r.byte()?;
        let preset_used = i32::read(&mut r)?;
        let party_states = if let Some(n) = bounded(&mut r, 1000)? {
            let mut states = Vec::with_capacity(n);
            for _ in 0..n {
                states.push(i32::read(&mut r)?)
            }
            Some(states)
        } else {
            None
        };
        Some(Message {
            characters,
            max_free,
            max_purchased,
            max_premium,
            prologue_played,
            preset_used,
            party_states,
        })
    };
    if !r.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(result)
}
pub fn decode_ingress(
    category: i32,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message<'_>>, CharacterListError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    if category == CATEGORY {
        return decode(body, max_body);
    }
    if category != 0 {
        return Err(CharacterListError::Type);
    }
    let guid = body.get(..16).ok_or(PayloadError::Truncated)?;
    let fields = &body[16..];
    if guid == TYPE_ID.as_bytes() {
        return decode(fields, max_body - 16);
    }
    if guid == uuid::Uuid::from_u128(0x81c5fe01027c3e1c98d5da9c9862aa21).as_bytes() && fields == [0]
    {
        return Ok(None);
    }
    Err(CharacterListError::Type)
}
/// CostumeUpdateMessage (447): a nullable message whose only field is CostumeInfo.
pub const COSTUME_UPDATE_CATEGORY: i32 = 447;
pub const COSTUME_UPDATE_TYPE_ID: uuid::Uuid =
    uuid::Uuid::from_u128(0x099b778f_141e_359d_b270_60c9d2f99bbb);
/// Outer None is a null message; inner None is a null CostumeInfo.
pub fn encode_costume_update(
    message: Option<Option<&Costume>>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, CharacterListError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = match message {
        None => w.append(&[0]).map_err(CharacterListError::from),
        Some(costume) => w
            .append(&[1])
            .map_err(CharacterListError::from)
            .and_then(|()| write_costume(&mut w, costume)),
    };
    if result.is_err() {
        w.bytes.zeroize();
    }
    result?;
    Ok(Zeroizing::new(w.bytes))
}
pub fn decode_costume_update(
    body: &[u8],
    max_body: usize,
) -> Result<Option<Option<Costume>>, CharacterListError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        Some(read_costume(&mut r)?)
    };
    if !r.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(result)
}
