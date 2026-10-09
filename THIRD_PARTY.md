# Third-party notices

## fdlibm (via musl)

`crates/rr_dr60_detmath` uses polynomial coefficients and argument-reduction constants from
fdlibm's `__kernel_sin`, `__kernel_cos`, `__rem_pio2`, `e_exp` and `e_log`, as adapted in musl
libc. The notice for that material:

> Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
>
> Developed at SunPro, a Sun Microsystems, Inc. business.
> Permission to use, copy, modify, and distribute this
> software is freely granted, provided that this notice
> is preserved.

This permissive notice is compatible with this project's MIT license. The same notice also
appears at the top of `crates/rr_dr60_detmath/src/lib.rs`.
