//! Experimental protocol design artifacts. No codec, RPC server, or IPC transport.

use std::fmt;

pub const SCHEMA: &str = include_str!("../proto/fqe/v1alpha1/engine.proto");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolVersion {
    pub major: u32,
    pub minor: u32,
}

pub const CURRENT_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };

/// Negotiates the version number only, not features, authentication, or a session.
pub fn negotiate(peer: ProtocolVersion) -> Result<ProtocolVersion, IncompatibleVersion> {
    if peer.major != CURRENT_VERSION.major {
        return Err(IncompatibleVersion {
            local: CURRENT_VERSION,
            peer,
        });
    }
    // Minor zero is the only implemented baseline; compatible peers use it.
    Ok(CURRENT_VERSION)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IncompatibleVersion {
    pub local: ProtocolVersion,
    pub peer: ProtocolVersion,
}

impl fmt::Display for IncompatibleVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "incompatible protocol major versions: local={}, peer={}",
            self.local.major, self.peer.major
        )
    }
}

impl std::error::Error for IncompatibleVersion {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_and_newer_minor_versions_negotiate_to_local_minor() {
        for minor in [0, 1, u32::MAX] {
            assert_eq!(
                negotiate(ProtocolVersion { major: 1, minor }),
                Ok(CURRENT_VERSION)
            );
        }
    }

    #[test]
    fn incompatible_majors_are_rejected() {
        for major in [0, 2, u32::MAX] {
            let peer = ProtocolVersion { major, minor: 0 };
            assert_eq!(
                negotiate(peer),
                Err(IncompatibleVersion {
                    local: CURRENT_VERSION,
                    peer,
                })
            );
        }
    }
}
