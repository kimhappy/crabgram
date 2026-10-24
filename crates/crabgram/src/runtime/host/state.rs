/// The state a `#[control(blockable)]` input takes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
#[repr(u32)]
pub enum ControlState {
    /// Removes any state set before.
    #[default]
    Normal = 0,
    /// Changing the value has no audible effect.
    Inactive = 1,
    /// The plugin ignores the value.
    Blocked = 2,
}

/// The state an input without `#[control(blockable)]` takes: whether changing it is heard.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum Activity {
    /// Removes any state set before.
    #[default]
    Active,
    /// Changing the value has no audible effect.
    Inactive,
}

impl From<Activity> for ControlState {
    fn from(activity: Activity) -> Self {
        match activity {
            Activity::Active => Self::Normal,
            Activity::Inactive => Self::Inactive,
        }
    }
}

impl ControlState {
    /// The state a host passes as `number`, if it names one.
    #[must_use]
    pub const fn from_number(number: u32) -> Option<Self> {
        match number {
            0 => Some(Self::Normal),
            1 => Some(Self::Inactive),
            2 => Some(Self::Blocked),
            _ => None,
        }
    }
}

impl From<ControlState> for u32 {
    /// The number a host receives as `state`.
    fn from(state: ControlState) -> Self {
        state as Self
    }
}

#[cfg(test)]
mod tests {
    use super::{Activity, ControlState};

    #[test]
    fn every_state_round_trips_through_its_number() {
        let states = [
            ControlState::Normal,
            ControlState::Inactive,
            ControlState::Blocked,
        ];

        assert!(
            states
                .into_iter()
                .all(|state| ControlState::from_number(state.into()) == Some(state))
        );
    }

    #[test]
    fn an_activity_is_the_state_of_its_name() {
        assert_eq!(ControlState::from(Activity::Active), ControlState::Normal);
        assert_eq!(
            ControlState::from(Activity::Inactive),
            ControlState::Inactive
        );
    }

    #[test]
    fn a_number_past_the_states_names_none() {
        assert_eq!(ControlState::from_number(3), None);
    }
}
