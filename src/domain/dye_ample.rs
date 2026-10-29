//! Recovered DyeAmpleSession and DistortTable: the empty dye ampoule's color table, the
//! picker seed, the DyeCashCursor grid and the colored ampoule. No storage or logs.
use super::item_colors::{ColorError, Materials};
use thiserror::Error;

/// DyeAmpleSession.EmptyAmpleHeader, in match order.
const EMPTY_HEADERS: [&str; 3] = ["dye_empty_event_", "dye_empty_avatar_", "dye_empty_"];
pub const COLOR_CLASS: &str = "dye_color_cash";
pub const AVATAR_COLOR_CLASS: &str = "dye_color_avatar_avatarcolor";
pub const NO_TRADE: &str = "_notrade";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DyeError {
    #[error("item is not an empty dye ampoule")]
    NotEmpty,
    #[error("dye cursor grid is negative")]
    Cursor,
    #[error("client cursor colors are fewer than the grid")]
    ColorValue,
    #[error(transparent)]
    Color(#[from] ColorError),
}

/// DyeAmpleSession.IsEmptyAmple.
pub fn is_empty(class: &str) -> bool {
    EMPTY_HEADERS.iter().any(|h| class.starts_with(h))
}

/// DyeAmpleSession.From: the color table after the first matching header; a text that
/// contains `_notrade` loses its last eight characters (String.Remove).
pub fn color_table(class: &str) -> Option<String> {
    let header = EMPTY_HEADERS.iter().find(|h| class.starts_with(*h))?;
    let text: Vec<u16> = class[header.len()..].encode_utf16().collect();
    let notrade: Vec<u16> = NO_TRADE.encode_utf16().collect();
    let text = if text.windows(notrade.len()).any(|w| w == notrade.as_slice()) {
        &text[..text.len() - notrade.len()]
    } else {
        &text[..]
    };
    Some(String::from_utf16_lossy(text))
}

/// Framework int.TryParse (NumberStyles.Integer): white space around, one sign, ASCII digits.
pub fn try_parse(text: &str) -> Option<i32> {
    let text = text.trim_matches(|c| matches!(c, ' ' | '\u{0009}'..='\u{000d}'));
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// DyeCashCursor "WxHxGap": three parts, each failed width/height parse is 1 and a
/// failed gap 0; another part count is 1x1x1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub width: i32,
    pub height: i32,
    pub gap: i32,
}
impl Cursor {
    pub fn parse(value: &str) -> Self {
        let parts: Vec<&str> = value.split('x').collect();
        if parts.len() != 3 {
            return Self {
                width: 1,
                height: 1,
                gap: 1,
            };
        }
        Self {
            width: try_parse(parts[0]).unwrap_or(1),
            height: try_parse(parts[1]).unwrap_or(1),
            gap: try_parse(parts[2]).unwrap_or(0),
        }
    }
}

/// DistortTable: 256 entries of dye.displace (ReadByte, -1 past the end of the file).
/// Missing entries (an unreadable file) read as 0 through TryGetValue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DistortTable {
    table: Option<[i32; 256]>,
}
impl DistortTable {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut table = [-1; 256];
        for (slot, byte) in table.iter_mut().zip(bytes) {
            *slot = i32::from(*byte);
        }
        Self { table: Some(table) }
    }
    /// The failed load: an empty dictionary.
    pub fn empty() -> Self {
        Self { table: None }
    }
    /// Test oracle tables: one entry per index, None for an absent key.
    pub fn from_entries(entries: &[Option<i32>; 256]) -> Option<Self> {
        if entries.iter().all(Option::is_none) {
            return Some(Self::empty());
        }
        let mut table = [0; 256];
        for (slot, entry) in table.iter_mut().zip(entries) {
            *slot = (*entry)?;
        }
        Some(Self { table: Some(table) })
    }
    fn get(&self, key: i32) -> i32 {
        match (&self.table, usize::try_from(key)) {
            (Some(t), Ok(k)) if k < 256 => t[k],
            _ => 0,
        }
    }
    fn shift(&self, other: i32, seed: i32, rate: i32) -> i32 {
        let num = self.get(other.wrapping_add(seed) % 256);
        num.wrapping_mul(rate) / 100 % 256
    }
    /// DistortTable.Undistort (Int32 unchecked, truncating remainders).
    pub fn undistort(&self, x: i32, y: i32, seed: [i32; 4]) -> (i32, i32) {
        let undistort = |value: i32, other: i32, seed: i32, rate: i32| {
            value
                .wrapping_add(self.shift(other, seed, rate))
                .wrapping_add(256)
                % 256
        };
        let y = undistort(y, x, seed[3], 3);
        let x = undistort(x, y, seed[2], 20);
        let y = undistort(y, x, seed[1], 30);
        let x = undistort(x, y, seed[0], 50);
        (x, y)
    }
}

/// ItemOwner.DyeAmpleSession of the selected character.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub item_id: i64,
    pub color_table: String,
    pub seed: [i32; 4],
    pub cursor: Cursor,
    /// ItemLog.AddItemJournal("DyeAmple") of the session.
    pub journal: i64,
}

/// The colored ampoule of MakeColorAmple.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Made {
    pub class: String,
    pub tradable: bool,
    pub color: i32,
    /// A client cursor color differed from the server's (logged, not refused).
    pub mismatch: bool,
}

impl Session {
    /// GetColor: Undistort, then ColorTable.PickColorInt(ColorTableName, x, y).
    pub fn color(
        &self,
        materials: &Materials,
        distort: &DistortTable,
        x: i32,
        y: i32,
    ) -> Result<i32, DyeError> {
        let (x, y) = distort.undistort(x, y, self.seed);
        let name: Vec<u16> = self.color_table.encode_utf16().collect();
        Ok(materials.pick_at(Some(&name), x, y)?)
    }

    /// MakeColorAmple. `pick` is Random.Next(CursorWidth) and Next(CursorHeight) of the
    /// source's fresh Random; `class`/`tradable` are the ampoule before the change.
    #[allow(clippy::too_many_arguments)]
    pub fn make(
        &self,
        materials: &Materials,
        distort: &DistortTable,
        class: &str,
        tradable: bool,
        x: i32,
        y: i32,
        is_avatar: bool,
        color_value: &[i32],
        pick: impl FnOnce(i32, i32) -> Result<(i32, i32), DyeError>,
    ) -> Result<Made, DyeError> {
        let Cursor { width, height, gap } = self.cursor;
        if width < 0 || height < 0 {
            return Err(DyeError::Cursor);
        }
        let mut mismatch = false;
        for i in 0..width {
            for j in 0..height {
                let server = self.color(
                    materials,
                    distort,
                    x.wrapping_add(i.wrapping_mul(gap)),
                    y.wrapping_add(j.wrapping_mul(gap)),
                )?;
                let index = usize::try_from(i.wrapping_mul(height).wrapping_add(j))
                    .map_err(|_| DyeError::ColorValue)?;
                let client = color_value.get(index).ok_or(DyeError::ColorValue)?;
                mismatch |= *client != server;
            }
        }
        let (i, j) = pick(width, height)?;
        let color = self.color(
            materials,
            distort,
            x.wrapping_add(i.wrapping_mul(gap)),
            y.wrapping_add(j.wrapping_mul(gap)),
        )?;
        let mut made = String::from(if is_avatar {
            AVATAR_COLOR_CLASS
        } else {
            COLOR_CLASS
        });
        let notrade = class.contains(NO_TRADE);
        if notrade {
            made.push_str(NO_TRADE);
        }
        Ok(Made {
            class: made,
            tradable: tradable && !notrade,
            color,
            mismatch,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_tables_and_cursor_follow_the_source() {
        assert!(is_empty("dye_empty_event_cash"));
        assert!(!is_empty("dye_color_cash"));
        assert_eq!(color_table("dye_empty_event_cash").as_deref(), Some("cash"));
        assert_eq!(
            color_table("dye_empty_avatar_avatarcolor_notrade").as_deref(),
            Some("avatarcolor")
        );
        assert_eq!(
            color_table("dye_empty_cash_new").as_deref(),
            Some("cash_new")
        );
        // Contains, then Remove the last eight characters wherever `_notrade` was.
        assert_eq!(color_table("dye_empty__notradeab").as_deref(), Some("_n"));
        assert_eq!(color_table("dye_lvl1"), None);
        assert_eq!(
            Cursor::parse("3x2x40"),
            Cursor {
                width: 3,
                height: 2,
                gap: 40
            }
        );
        assert_eq!(
            Cursor::parse("ax2xb"),
            Cursor {
                width: 1,
                height: 2,
                gap: 0
            }
        );
        assert_eq!(
            Cursor::parse(""),
            Cursor {
                width: 1,
                height: 1,
                gap: 1
            }
        );
    }

    #[test]
    fn the_colored_ampoule_keeps_notrade_and_checks_the_client_grid() {
        let mut materials = Materials::default();
        let raw: Vec<u8> = (0..=255u8).flat_map(|v| [v, 0, 255 - v]).collect();
        let name: Vec<u16> = "cash".encode_utf16().collect();
        materials.insert(&name, Some(&raw)).unwrap();
        let session = Session {
            item_id: 1,
            color_table: "cash".into(),
            seed: [1, 2, 3, 4],
            cursor: Cursor::parse("3x2x40"),
            journal: 0,
        };
        let distort = DistortTable::empty();
        let grid: Vec<i32> = (0..3)
            .flat_map(|i| (0..2).map(move |j| (i, j)))
            .map(|(i, j)| {
                session
                    .color(&materials, &distort, 5 + i * 40, 7 + j * 40)
                    .unwrap()
            })
            .collect();
        let made = session
            .make(
                &materials,
                &distort,
                "dye_empty_cash_notrade",
                true,
                5,
                7,
                false,
                &grid,
                |_, _| Ok((2, 1)),
            )
            .unwrap();
        assert_eq!(made.class, "dye_color_cash_notrade");
        assert!(!made.tradable && !made.mismatch);
        assert_eq!(made.color, grid[2 * 2 + 1]);
        let avatar = session
            .make(
                &materials,
                &distort,
                "dye_empty_event_cash",
                true,
                5,
                7,
                true,
                &[0; 6],
                |_, _| Ok((0, 0)),
            )
            .unwrap();
        assert_eq!(avatar.class, AVATAR_COLOR_CLASS);
        assert!(avatar.tradable && avatar.mismatch);
        assert_eq!(
            session.make(
                &materials,
                &distort,
                "dye_empty_event_cash",
                true,
                5,
                7,
                false,
                &grid[..5],
                |_, _| Ok((0, 0)),
            ),
            Err(DyeError::ColorValue)
        );
    }
}
