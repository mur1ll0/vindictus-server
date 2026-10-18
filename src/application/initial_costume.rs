//! Constructor-time CostumeManager snapshot. No Save,item overlay,callbacks or frontend.
use crate::{
    application::lobby_costume,
    domain::character_costume::{Costume, CostumeProfile, CostumeSet},
    protocol::character_list::Costume as Info,
};
#[derive(Clone, Copy)]
pub struct Context {
    pub cafe_type: i32,
    pub is_return: bool,
}
type Groups = Vec<(u8, Vec<Costume>)>;
pub struct Loaded {
    base: Groups,
    item: Groups,
    info: Info,
}
impl Loaded {
    pub fn load(rows: CostumeSet, context: Context) -> Result<Self, CostumeProfile> {
        let mut info = lobby_costume::project(&rows)?;
        info.cafe_type = context.cafe_type;
        info.is_return = context.is_return;
        let (mut base, mut item) = (Groups::new(), Groups::new());
        for row in rows.rows() {
            let groups = if row.is_base { &mut base } else { &mut item };
            if let Some((_, entries)) = groups.iter_mut().find(|(kind, _)| *kind == row.kind) {
                entries.push(*row)
            } else {
                groups.push((row.kind, vec![*row]))
            }
        }
        Ok(Self { base, item, info })
    }
    pub fn info(&self) -> &Info {
        &self.info
    }
    pub fn base_rows(&self) -> impl Iterator<Item = &Costume> {
        self.base.iter().flat_map(|(_, rows)| rows.iter())
    }
    pub fn item_rows(&self) -> impl Iterator<Item = &Costume> {
        self.item.iter().flat_map(|(_, rows)| rows.iter())
    }
}
