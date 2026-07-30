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

    /// The most literal take on "kaleidoscope": after computing the
    /// reflected/refracted direction, its azimuthal angle around the world
    /// up-axis is folded (mirrored, triangle-wave style) into one of six
    /// repeating wedges - exactly how a real optical kaleidoscope's ring of
    /// angled mirrors works, just applied to ray directions instead of
    /// light rays in a tube. Unlike the axis/normal-corruption bugs above,
    /// this one is a plausible "helper function reused from the wrong
    /// context" mistake (e.g. a stray call to a UV-tiling utility) rather
    /// than a sign/typo error, but it produces the cleanest, most
    /// rotationally-symmetric tiling of the bunch.
    AngularFold,

    /// The real bug, found in the original source (2013, C++). In the
    /// shader's `getHitColor`, a locally-declared `Vector3D rCol;` inside
    /// the mirror-reflection branch *shadows* the outer accumulator of the
    /// same name - so the recursively-computed reflection color gets summed
    /// into a variable that's discarded when the block ends, and the real
    /// `rCol` used later is always `(0,0,0)`. But the hit-record used to
    /// compute that reflection is passed by mutable reference and gets
    /// updated in place regardless - so the *only* surviving effect of
    /// "reflecting" is that the shading point silently drifts to wherever
    /// the reflected ray landed, for every remaining iteration of an inner
    /// per-light sample loop, and even across lights and nested recursive
    /// calls (all sharing one mutable hit-record and one shared, never-reset
    /// bounce budget). The final color ends up being local diffuse lighting
    /// sampled at a chaotic, order-dependent walk across the scene's
    /// geometry - not real reflection at all. Two more original quirks
    /// carried over faithfully: `abs(N.L)` instead of `max(0, N.L)` (so
    /// back-facing samples light up too), and the diffuse/mirror blend
    /// factor being (re-)applied once per light rather than once overall, so
    /// it compounds multiplicatively in multi-light scenes.
    HitChainDrift,

    /// The original source's *other* function, `mapLightRay` (presumably a
    /// forward light-tracing / photon-mapping path, never fully ported here,
    /// since that would need real photon storage and forward tracing from
    /// lights, a different architecture entirely). Its bugs are simple
    /// enough to borrow directly on top of our normal backward ray tracing.
    /// First: the local lighting term dots the *incident ray direction*
    /// with the normal without negating it first, so ordinary front-lit
    /// surfaces (where incoming and normal oppose each other) go dark, and
    /// only grazing/back-facing geometry lights up. Second: bounces
    /// terminate on an unweighted coin flip (`rand() > 0.5`) with no
    /// compensating survival weight, the textbook way to make Russian
    /// roulette biased instead of unbiased, so brightness varies noisily
    /// sample to sample. Third: a reflection ray that hits nothing returns
    /// a literal `(-1,-1,-1)` sentinel that - unlike the "stop" branch -
    /// never gets clamped back into range, so it bleeds through the
    /// recursive color math as-is, occasionally flipping a product's sign
    /// into an impossible bright pixel where two negatives multiply
    /// positive.
    CoinFlipMiss,
}

impl GlitchMode {
    pub const ALL: [GlitchMode; 12] = [
        GlitchMode::None,
        GlitchMode::KaleidoscopeStaleDirection,
        GlitchMode::KaleidoscopeStaleNormal,
        GlitchMode::FlippedReflectSign,
        GlitchMode::AxisSwapReflect,
        GlitchMode::InvertedFresnel,
        GlitchMode::SwappedEta,
        GlitchMode::EnergyBleed,
        GlitchMode::NormalDrift,
        GlitchMode::AngularFold,
        GlitchMode::HitChainDrift,
        GlitchMode::CoinFlipMiss,
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
            GlitchMode::AngularFold => "09_angular_fold",
            GlitchMode::HitChainDrift => "10_hit_chain_drift",
            GlitchMode::CoinFlipMiss => "11_coin_flip_miss",
        }
    }

    /// CLI/config-facing name (kebab-case, no ordinal prefix).
    pub fn name(self) -> &'static str {
        match self {
            GlitchMode::None => "none",
            GlitchMode::KaleidoscopeStaleDirection => "kaleidoscope-stale-direction",
            GlitchMode::KaleidoscopeStaleNormal => "kaleidoscope-stale-normal",
            GlitchMode::FlippedReflectSign => "flipped-reflect-sign",
            GlitchMode::AxisSwapReflect => "axis-swap-reflect",
            GlitchMode::InvertedFresnel => "inverted-fresnel",
            GlitchMode::SwappedEta => "swapped-eta",
            GlitchMode::EnergyBleed => "energy-bleed",
            GlitchMode::NormalDrift => "normal-drift",
            GlitchMode::AngularFold => "angular-fold",
            GlitchMode::HitChainDrift => "hit-chain-drift",
            GlitchMode::CoinFlipMiss => "coin-flip-miss",
        }
    }

    pub fn from_name(name: &str) -> Option<GlitchMode> {
        GlitchMode::ALL.into_iter().find(|m| m.name() == name)
    }
}

impl std::str::FromStr for GlitchMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        GlitchMode::from_name(s).ok_or_else(|| {
            let names: Vec<_> = GlitchMode::ALL.iter().map(|m| m.name()).collect();
            format!("unknown glitch mode {s:?}; expected one of: {}", names.join(", "))
        })
    }
}

impl std::fmt::Display for GlitchMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
