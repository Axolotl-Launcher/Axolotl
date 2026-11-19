//! Availability belongs to one document; callers decide how its service degrades.

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Availability {
    Missing,
    Ready,
    Recovered,
    /// The original bytes are intact, but this reader cannot safely change them.
    ReadOnly(String),
    /// Neither the published document nor its previous good copy can be read.
    Unavailable(String),
}

impl Availability {
    pub(crate) fn writable(&self) -> bool {
        matches!(self, Self::Missing | Self::Ready | Self::Recovered)
    }
}
