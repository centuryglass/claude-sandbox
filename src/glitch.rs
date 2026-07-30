/// Deliberate departures from physically-correct light transport, selectable
/// per render. Each one is a plausible "what if I got this one line wrong"
/// bug in the reflection/refraction shader - the point of this module is to
/// break things on purpose and see what's beautiful about the breakage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlitchMode {
    #[default]
    None,

    /// The original bug this whole module is chasing: recursive mirror
    /// bounces reflect the *primary* camera ray's direction instead of the
    /// current bounce's incoming direction (a stale-variable bug - reuse the
    /// outer ray instead of the one you just spawned). Every reflective
    /// surface ends up showing the same view direction refolded around its
    /// own normal, and because it compounds bounce over bounce, flat facets
    /// tile the same folded image into kaleidoscopic mirror copies.
    KaleidoscopeStaleDirection,

    /// A close cousin of the bug above, and maybe closer to what actually
    /// happened: the surface normal used to compute the reflection axis is
    /// frozen at the *first* bounce and reused for every subsequent one,
    /// instead of being re-fetched at each hit (a stale-variable bug one
    /// scope level removed from the one above). Ray origins still update
    /// correctly from real geometry each bounce, so the path stays
    /// position-dependent - but every turn happens around the same fixed
    /// axis, so light ping-ponging through a faceted cluster folds into
    /// dense, self-similar tiling instead of true reflections.
    KaleidoscopeStaleNormal,

    /// The smallest possible typo: `d + 2(d.n)n` instead of `d - 2(d.n)n` in
    /// the reflect formula. Sign errors like this are exactly the kind of
    /// thing that survives a quick glance at the code. Instead of bouncing
    /// away from a surface, "reflected" rays get pushed further into it -
    /// so inside a convex faceted solid (like a cut gem) they ricochet off
    /// the *inside* of neighboring facets over and over before the depth
    /// limit cuts them off, sampling many different facet normals along the
    /// way. That reads as dense, tiled internal faceting concentrated inside
    /// the gem itself, rather than a scene-wide fold.
    FlippedReflectSign,

    /// A one-character typo: cyclically permute the reflected vector's
    /// components (x,y,z) -> (y,z,x) before tracing the bounce. Reflections
    /// still land on the mirror surface but everything behind it appears
    /// sheared into a different axis, tiling hard geometric facets into
    /// pinwheel-like repeats.
    AxisSwapReflect,

    /// Schlick's approximation inverted: total-internal-reflection edges go
    /// transparent and face-on glass goes mirrored, i.e. exactly backwards
    /// from real Fresnel behavior. Glass reads like a strange X-ray/ghost
    /// material with reflections in the middle and see-through rims.
    InvertedFresnel,

    /// Refraction ratio (ior_incident / ior_transmitted) computed with the
    /// front/back test flipped, so rays entering glass bend as if leaving it
    /// and vice versa. Solid objects appear to invert themselves - convex
    /// surfaces refract like concave ones - giving a warped, turned-inside-
    /// out look.
    SwappedEta,

    /// Reflected/refracted light is *added* to the surface color instead of
    /// attenuated (multiplied) by it. Energy conservation goes out the
    /// window: every bounce brightens the result instead of dimming it, so
    /// mirror clusters glow and bloom into neon rather than going dark.
    EnergyBleed,

    /// Simulates a "forgot to renormalize the surface normal" bug (e.g.
    /// after a non-uniform scale, or a botched vertex-normal interpolation):
    /// the shading normal's length is corrupted by a smooth function of the
    /// hit position before it's used for lighting/reflection/refraction.
    /// Unlike the tiling glitches above this one is spatially organic -
    /// surfaces ripple and melt rather than fold into copies.
    NormalDrift,
}

impl GlitchMode {
    pub const ALL: [GlitchMode; 9] = [
        GlitchMode::None,
        GlitchMode::KaleidoscopeStaleDirection,
        GlitchMode::KaleidoscopeStaleNormal,
        GlitchMode::FlippedReflectSign,
        GlitchMode::AxisSwapReflect,
        GlitchMode::InvertedFresnel,
        GlitchMode::SwappedEta,
        GlitchMode::EnergyBleed,
        GlitchMode::NormalDrift,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            GlitchMode::None => "00_reference",
            GlitchMode::KaleidoscopeStaleDirection => "01_kaleidoscope_stale_direction",
            GlitchMode::KaleidoscopeStaleNormal => "02_kaleidoscope_stale_normal",
            GlitchMode::FlippedReflectSign => "03_flipped_reflect_sign",
            GlitchMode::AxisSwapReflect => "04_axis_swap_reflect",
            GlitchMode::InvertedFresnel => "05_inverted_fresnel",
            GlitchMode::SwappedEta => "06_swapped_eta",
            GlitchMode::EnergyBleed => "07_energy_bleed",
            GlitchMode::NormalDrift => "08_normal_drift",
        }
    }
}
