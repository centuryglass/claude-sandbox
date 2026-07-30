# Original source

`MultiReflectionShader.cpp`/`.h`, written May 2013 for a college graphics
course. This is the actual shader behind the original kaleidoscope-fold
artifacts that `hit-chain-drift` (see `../src/render.rs`, `../src/glitch.rs`)
reverse-engineers and reproduces on purpose. Kept verbatim, bugs and all -
see the top-level README's "The real bug" section for the writeup of what's
actually going on in `getHitColor`.

Note: `MultiReflectionShader.cpp` does `#include "AreaLight.h"` and calls
`AreaLight::getLightPt` - that class was never uploaded/preserved here,
only inferred from its call site. `Light::Rect` in `../src/light.rs` (see
the top-level README's "Area lights and soft shadows" section) is this
project's own implementation of the same jittered-point-per-ray idea, not
a port of unseen code.
