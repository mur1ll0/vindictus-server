//! Recovered RGB materials, Framework color selection and Item.SetColor.
//! Immutable content snapshot; no filesystem watcher, item writes or implicit clock.
use thiserror::Error;
use zeroize::Zeroizing;
pub const WHITE: i32 = 0x00ff_ffff;
const MAX_RAW: usize = 196_610;
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ColorError {
    #[error("color content exceeds verified bounds or has duplicate keys")]
    Bound,
    #[error("material is absent from the bound content snapshot")]
    UnboundMaterial,
    #[error("coordinate selection has an empty color table")]
    EmptyCoordinates,
    #[error("coordinate selection produced a negative source index")]
    CoordinateIndex,
    #[error("look class does not have equipment information")]
    MissingLookEquipment,
    #[error("random maximum is negative")]
    RandomMaximum,
}
pub fn rgb_to_int(rgb: [u8; 3]) -> i32 {
    (i32::from(rgb[0]) << 16) | (i32::from(rgb[1]) << 8) | i32::from(rgb[2])
}
pub fn int_to_rgb(color: i32) -> [u8; 3] {
    [(color >> 16) as u8, (color >> 8) as u8, color as u8]
}
pub struct Material {
    colors: Zeroizing<Vec<i32>>,
}
impl Material {
    pub fn from_raw(raw: &[u8]) -> Result<Self, ColorError> {
        if raw.len() > MAX_RAW {
            return Err(ColorError::Bound);
        }
        Ok(Self {
            colors: Zeroizing::new(
                raw.as_chunks::<3>()
                    .0
                    .iter()
                    .map(|b| rgb_to_int([b[0], b[1], b[2]]))
                    .collect(),
            ),
        })
    }
    pub fn colors(&self) -> &[i32] {
        &self.colors
    }
    pub fn pick(&self, random: &mut ColorRandom) -> i32 {
        if self.colors.is_empty() {
            return WHITE;
        }
        self.colors[random.next_unchecked(self.colors.len() as i32) as usize]
    }
    pub fn pick_at(&self, x: i32, y: i32) -> Result<i32, ColorError> {
        if self.colors.is_empty() {
            return Err(ColorError::EmptyCoordinates);
        }
        let index = x.wrapping_add(256_i32.wrapping_mul(y)) % self.colors.len() as i32;
        let index = usize::try_from(index).map_err(|_| ColorError::CoordinateIndex)?;
        self.colors
            .get(index)
            .copied()
            .ok_or(ColorError::CoordinateIndex)
    }
}
/// Separate from password RNG: this is the original shared material generator.
/// Seed is explicit; the caller must recover process TickCount when integrating startup.
pub struct ColorRandom {
    seed: Zeroizing<[i32; 56]>,
    next: usize,
    nextp: usize,
}
impl ColorRandom {
    pub fn from_seed(value: i32) -> Self {
        let absolute = if value == i32::MIN {
            i32::MAX
        } else {
            value.abs()
        };
        let mut seed = Zeroizing::new([0_i32; 56]);
        let mut mj = 161803398_i32.wrapping_sub(absolute);
        seed[55] = mj;
        let mut mk = 1_i32;
        for i in 1..55 {
            let index = 21 * i % 55;
            seed[index] = mk;
            mk = mj.wrapping_sub(mk);
            if mk < 0 {
                mk = mk.wrapping_add(i32::MAX);
            }
            mj = seed[index];
        }
        for _ in 1..5 {
            for k in 1..56 {
                seed[k] = seed[k].wrapping_sub(seed[1 + (k + 30) % 55]);
                if seed[k] < 0 {
                    seed[k] = seed[k].wrapping_add(i32::MAX);
                }
            }
        }
        Self {
            seed,
            next: 0,
            nextp: 21,
        }
    }
    /// InternalSample.
    fn sample(&mut self) -> i32 {
        self.next += 1;
        if self.next >= 56 {
            self.next = 1;
        }
        self.nextp += 1;
        if self.nextp >= 56 {
            self.nextp = 1;
        }
        let mut sample = self.seed[self.next].wrapping_sub(self.seed[self.nextp]);
        if sample == i32::MAX {
            sample -= 1;
        }
        if sample < 0 {
            sample = sample.wrapping_add(i32::MAX);
        }
        self.seed[self.next] = sample;
        sample
    }
    fn next_unchecked(&mut self, maximum: i32) -> i32 {
        ((f64::from(self.sample()) * 4.656612875245797E-10) * f64::from(maximum)) as i32
    }
    /// Random.NextDouble (Sample()).
    pub fn next_double(&mut self) -> f64 {
        f64::from(self.sample()) * 4.656612875245797E-10
    }
    /// Next(0) consumes one sample, as the Framework does. Negative maxima do not.
    pub fn next(&mut self, maximum: i32) -> Result<i32, ColorError> {
        if maximum < 0 {
            return Err(ColorError::RandomMaximum);
        }
        Ok(self.next_unchecked(maximum))
    }
}
#[derive(Default)]
pub struct Materials {
    entries: Vec<(Zeroizing<Vec<u16>>, Option<Material>)>,
    bytes: usize,
}
impl Materials {
    /// None is a recovered missing/unreadable file, cached as null by ColorTable.
    /// An unlisted name fails: it is not evidence that the original file was missing.
    pub fn insert(&mut self, name: &[u16], raw: Option<&[u8]>) -> Result<(), ColorError> {
        if name.len() > 256
            || self.entries.len() >= 256
            || self.entries.iter().any(|(k, _)| k.as_slice() == name)
            || self.bytes + raw.map_or(0, |v| v.len()) > 8 * 1024 * 1024
        {
            return Err(ColorError::Bound);
        }
        let material = raw.map(Material::from_raw).transpose()?;
        self.bytes += raw.map_or(0, |v| v.len());
        self.entries.push((Zeroizing::new(name.to_vec()), material));
        Ok(())
    }
    pub fn get(&self, name: &[u16]) -> Result<Option<&Material>, ColorError> {
        self.entries
            .iter()
            .find(|(k, _)| k.as_slice() == name)
            .map(|(_, v)| v.as_ref())
            .ok_or(ColorError::UnboundMaterial)
    }
    pub fn pick(&self, name: Option<&[u16]>, random: &mut ColorRandom) -> Result<i32, ColorError> {
        let Some(name) = name else {
            return Ok(WHITE);
        };
        Ok(self.get(name)?.map_or(WHITE, |m| m.pick(random)))
    }
    pub fn pick_at(&self, name: Option<&[u16]>, x: i32, y: i32) -> Result<i32, ColorError> {
        let Some(name) = name else {
            return Ok(WHITE);
        };
        self.get(name)?.map_or(Ok(WHITE), |m| m.pick_at(x, y))
    }
}
pub enum Look<'a> {
    Absent,
    WithoutEquipment,
    Equipment([Option<&'a [u16]>; 3]),
}
/// Returns whether Update(Color) must be called. Source exceptions retain earlier
/// channel mutations and suppress the final update; errors are not converted to white.
pub fn set_colors(
    current: &mut [i32; 3],
    requested: [Option<i32>; 3],
    equippable: bool,
    look: Look<'_>,
    materials: &Materials,
    random: &mut ColorRandom,
) -> Result<bool, ColorError> {
    if !equippable {
        return Ok(false);
    }
    for (channel, value) in requested.into_iter().enumerate() {
        let Some(value) = value else {
            continue;
        };
        if value != -1 {
            current[channel] = value;
            continue;
        }
        let names = match &look {
            Look::Absent => continue,
            Look::WithoutEquipment => return Err(ColorError::MissingLookEquipment),
            Look::Equipment(names) => names,
        };
        // The original skips a null Material1, but calls PickColorInt for 2 and 3.
        if channel == 0 && names[channel].is_none() {
            continue;
        }
        current[channel] = materials.pick(names[channel], random)?;
    }
    Ok(true)
}
