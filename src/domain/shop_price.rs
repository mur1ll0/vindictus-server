//! PlayerService.ItemServiceCore.PriceCalc with FeatureMatrix.GetFloat("ShopPriceRatio").
//! The Framework JIT rounds the Single products differently per architecture: x64 rounds
//! each product to Single, x86 keeps x87 extended precision. x86 is reproduced exactly
//! only for ratio 1 (every product is then an exact integer); other ratios are refused.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arch {
    X86,
    X64,
}

/// (int)double conversion of the JIT: out-of-range and NaN give int.MinValue.
fn to_int(value: f64) -> i32 {
    if value.is_nan() || value < i32::MIN as f64 || value >= 2_147_483_648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

/// PriceCalc.Ceiling(basePrice, ratio).
pub fn ceiling(base: i32, ratio: f32, arch: Arch) -> Option<i32> {
    let floor = match arch {
        Arch::X64 => ((base as f32 * ratio) * 1000f32) as f64,
        Arch::X86 if ratio == 1.0 => (base as f32) as f64 * 1000.0,
        Arch::X86 => return None,
    }
    .floor();
    Some(to_int((floor / 1000.0).ceil()))
}

/// PriceCalc.Ceiling10: (int)Math.Ceiling((float)Ceiling(...) / 10f) * 10, unchecked.
pub fn ceiling10(base: i32, ratio: f32, arch: Arch) -> Option<i32> {
    let c = ceiling(base, ratio, arch)? as f32;
    let quotient = match arch {
        Arch::X64 => (c / 10f32) as f64,
        // Extended precision of a Single over 10 rounds to double without loss here.
        Arch::X86 => c as f64 / 10.0,
    };
    Some(to_int(quotient.ceil()).wrapping_mul(10))
}

/// FeatureMatrix.GetFloat (float.Parse, en-US), with ItemContents replacing 0 by 1.
pub fn ratio(text: &str) -> Option<f32> {
    let parsed: f32 = if text.is_empty() {
        0.0
    } else {
        text.trim().parse().ok()?
    };
    Some(if parsed == 0.0 { 1.0 } else { parsed })
}
