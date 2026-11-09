//! Flushing denormal floats to zero while the plugin runs.

#[cfg(target_arch = "x86_64")]
mod control {
    use core::arch::asm;

    /// MXCSR.
    pub type Register = u32;

    /// Flush to zero (bit 15) and denormals are zero (bit 6).
    const FLUSH: Register = 1 << 15 | 1 << 6;

    fn read() -> Register {
        let mut register: Register = 0;

        // SAFETY: `stmxcsr` only stores MXCSR into the four bytes of `register`.
        unsafe {
            asm!("stmxcsr [{}]", in(reg) &raw mut register, options(nostack, preserves_flags));
        }
        register
    }

    pub fn restore(register: Register) {
        // SAFETY: `ldmxcsr` only loads MXCSR from the four bytes of `register`, a value `read`
        // returned with at most the flush bits set on top.
        unsafe {
            asm!(
                "ldmxcsr [{}]",
                in(reg) &raw const register,
                options(nostack, readonly, preserves_flags)
            );
        }
    }

    pub fn flush() -> Register {
        let previous = read();

        restore(previous | FLUSH);
        previous
    }
}

#[cfg(target_arch = "aarch64")]
mod control {
    use core::arch::asm;

    /// FPCR.
    pub type Register = u64;

    /// Flush to zero (bit 24).
    const FLUSH: Register = 1 << 24;

    fn read() -> Register {
        let register;

        // SAFETY: reading FPCR has no other effect.
        unsafe {
            asm!("mrs {}, fpcr", out(reg) register, options(nomem, nostack, preserves_flags));
        }
        register
    }

    pub fn restore(register: Register) {
        // SAFETY: `register` is a value `read` returned with at most the flush bit set on top.
        unsafe {
            asm!("msr fpcr, {}", in(reg) register, options(nostack, preserves_flags));
        }
    }

    pub fn flush() -> Register {
        let previous = read();

        restore(previous | FLUSH);
        previous
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
mod control {
    pub type Register = ();

    pub const fn restore((): Register) {}

    pub const fn flush() -> Register {}
}

/// Flushes denormals to zero on this thread until dropped, then puts the previous mode back.
///
/// Rust assumes the default floating-point environment, so code run while this lives is outside
/// what the language promises; it only does arithmetic, whose results the flush may change.
pub struct FlushDenormals {
    previous: control::Register,
}

impl FlushDenormals {
    #[must_use]
    pub fn begin() -> Self {
        Self {
            previous: control::flush(),
        }
    }
}

impl Drop for FlushDenormals {
    fn drop(&mut self) {
        control::restore(self.previous);
    }
}

#[cfg(all(test, any(target_arch = "x86_64", target_arch = "aarch64")))]
mod tests {
    use core::hint::black_box;

    use super::FlushDenormals;

    fn halved_smallest_normal() -> f32 {
        black_box(f32::MIN_POSITIVE) * black_box(0.5)
    }

    #[test]
    fn denormals_flush_to_zero_only_while_the_guard_lives() {
        assert!(halved_smallest_normal().is_subnormal());

        {
            let _flushing = FlushDenormals::begin();

            assert_eq!(halved_smallest_normal(), 0.0);
        }

        assert!(halved_smallest_normal().is_subnormal());
    }
}
