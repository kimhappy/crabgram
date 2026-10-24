//! Typed handles to the controls of a [`Controls`] struct.

use core::{fmt, marker::PhantomData};

use crate::{Control, Controls};

/// What a plugin may do with a [`Field`].
pub mod access {
    /// The access of a [`Field`](super::Field).
    pub trait Access: crate::sealed::SealedAccess + 'static {}

    /// A field the plugin may move with
    /// [`Context::request_change`](crate::Context::request_change): an input.
    #[diagnostic::on_unimplemented(
        message = "`request_change` moves an input, and this field is an output",
        note = "only the plugin writes an output: use `Context::write`"
    )]
    pub trait Requestable: Access {}

    /// A field the plugin may put in a state with
    /// [`Context::set_state`](crate::Context::set_state): an input.
    #[diagnostic::on_unimplemented(
        message = "`set_state` takes an input, and this field is an output"
    )]
    pub trait Stateful: Access {
        /// An [`Activity`](crate::Activity), or a [`ControlState`](crate::ControlState) for a
        /// `blockable` input.
        type State: Copy + Into<crate::ControlState>;

        /// The state the host holds, as one of [`State`](Self::State).
        #[doc(hidden)]
        fn state_of(state: crate::ControlState) -> Self::State;
    }

    /// A field the plugin may set with [`Context::write`](crate::Context::write): an output.
    #[diagnostic::on_unimplemented(
        message = "`write` sets an output, and this field is an input",
        note = "ask the host to move an input with `Context::request_change`"
    )]
    pub trait Writable: Access {}

    /// The access of an input.
    #[derive(Clone, Copy, Debug)]
    pub struct Input;

    /// The access of a `blockable` input.
    #[derive(Clone, Copy, Debug)]
    pub struct Blockable;

    /// The access of an output.
    #[derive(Clone, Copy, Debug)]
    pub struct Output;

    impl crate::sealed::SealedAccess for Input {}
    impl crate::sealed::SealedAccess for Blockable {}
    impl crate::sealed::SealedAccess for Output {}

    impl Access for Input {}
    impl Access for Blockable {}
    impl Access for Output {}

    impl Writable for Output {}

    impl Requestable for Input {}
    impl Requestable for Blockable {}

    impl Stateful for Input {
        type State = crate::Activity;

        fn state_of(state: crate::ControlState) -> crate::Activity {
            match state {
                crate::ControlState::Inactive => crate::Activity::Inactive,
                crate::ControlState::Normal | crate::ControlState::Blocked => {
                    crate::Activity::Active
                }
            }
        }
    }

    impl Stateful for Blockable {
        type State = crate::ControlState;

        fn state_of(state: crate::ControlState) -> crate::ControlState {
            state
        }
    }
}

/// The owner, access and value type of a [`Field`], none of which it holds.
type Marker<C, A, V> = fn() -> (C, A, V);

/// One control of the [`Controls`] struct `C`, typed by access `A` and
/// [`Value`](crate::Value) type `V`.
///
/// [`#[derive(Controls)]`](derive@crate::Controls) writes one per field (`Params::GAIN`).
///
/// ```
/// use crabgram::{Controls, Field, access::Blockable};
///
/// #[derive(Controls)]
/// struct Params {
///     #[control(blockable)]
///     depth: f32,
/// }
///
/// let depth: Field<Params, Blockable, f32> = Params::DEPTH;
///
/// assert_eq!(depth.as_ref().symbol(), "depth");
/// assert_eq!(Params::CONTROLS, [*depth.as_ref()]);
/// ```
///
/// A field of the wrong access, value type or plugin does not compile.
pub struct Field<C, A, V> {
    position: usize,
    marker: PhantomData<Marker<C, A, V>>,
}

impl<C: Controls, A: access::Access, V> Field<C, A, V> {
    pub(crate) const fn control(self) -> &'static Control {
        &C::CONTROLS[self.position]
    }
}

impl<C, A, V> Field<C, A, V> {
    pub(crate) const fn position(self) -> usize {
        self.position
    }
}

/// The field at `position` among the controls of `C`, for the derive macro only.
#[doc(hidden)]
#[must_use]
pub const fn make<C, A: access::Access, V>(position: usize) -> Field<C, A, V> {
    Field {
        position,
        marker: PhantomData,
    }
}

impl<C, A, V> Clone for Field<C, A, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C, A, V> Copy for Field<C, A, V> {}

/// Fields are equal when they are the same control.
impl<C, A, V> PartialEq for Field<C, A, V> {
    fn eq(&self, other: &Self) -> bool {
        self.position == other.position
    }
}

impl<C, A, V> Eq for Field<C, A, V> {}

/// The control a field stands for: `Params::GAIN.as_ref().symbol()`.
impl<C: Controls, A: access::Access, V> AsRef<Control> for Field<C, A, V> {
    fn as_ref(&self) -> &Control {
        self.control()
    }
}

impl<C: Controls, A: access::Access, V> fmt::Debug for Field<C, A, V> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Field")
            .field("control", self.control())
            .field("position", &self.position)
            .finish()
    }
}
