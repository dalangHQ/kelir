// see: cans/02-plugin-model.md#Unified-verdict
use std::fmt;

use serde::{Deserialize, Serialize};

// see: cans/02-plugin-model.md#Unified-verdict
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Errno {
    Eperm,
    Eacces,
    Enoent,
    Einval,
    Enospc,
    Erofs,
    Econnrefused,
    Enxio,
    Ewouldblock,
    Efault,
    Ebusy,
    Exdev,
    Epipe,
    Enodata,
    Eproto,
    Epolicy,
    Etoolquota,
    Ewxviolation,
    #[serde(rename = "EConfig")]
    Econfig,
    #[serde(rename = "ESecurityViolation")]
    Esecurityviolation,
    #[serde(rename = "EUnknownKind")]
    Eunknownkind,
    #[serde(rename = "EClockRegress")]
    Eclockregress,
    #[serde(rename = "PathTraversal")]
    PathTraversal,
}

impl Errno {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Eperm => "EPERM",
            Self::Eacces => "EACCES",
            Self::Enoent => "ENOENT",
            Self::Einval => "EINVAL",
            Self::Enospc => "ENOSPC",
            Self::Erofs => "EROFS",
            Self::Econnrefused => "ECONNREFUSED",
            Self::Enxio => "ENXIO",
            Self::Ewouldblock => "EWOULDBLOCK",
            Self::Efault => "EFAULT",
            Self::Ebusy => "EBUSY",
            Self::Exdev => "EXDEV",
            Self::Epipe => "EPIPE",
            Self::Enodata => "ENODATA",
            Self::Eproto => "EPROTO",
            Self::Epolicy => "EPOLICY",
            Self::Etoolquota => "ETOOLQUOTA",
            Self::Ewxviolation => "EWXVIOLATION",
            Self::Econfig => "EConfig",
            Self::Esecurityviolation => "ESecurityViolation",
            Self::Eunknownkind => "EUnknownKind",
            Self::Eclockregress => "EClockRegress",
            Self::PathTraversal => "PathTraversal",
        }
    }

    // see: cans/02-plugin-model.md#Unified-verdict
    pub fn to_os_errno(&self) -> i32 {
        match self {
            Self::Eperm => libc::EPERM,
            Self::Eacces => libc::EACCES,
            Self::Enoent => libc::ENOENT,
            Self::Einval => libc::EINVAL,
            Self::Enospc => libc::ENOSPC,
            Self::Erofs => libc::EROFS,
            Self::Econnrefused => libc::ECONNREFUSED,
            Self::Enxio => libc::ENXIO,
            Self::Ewouldblock => libc::EWOULDBLOCK,
            Self::Efault => libc::EFAULT,
            Self::Ebusy => libc::EBUSY,
            Self::Exdev => libc::EXDEV,
            Self::Epipe => libc::EPIPE,
            Self::Enodata => libc::ENODATA,
            Self::Eproto => libc::EPROTO,
            Self::PathTraversal | Self::Epolicy | Self::Esecurityviolation | Self::Ewxviolation => {
                libc::EPERM
            }
            Self::Etoolquota | Self::Econfig | Self::Eunknownkind | Self::Eclockregress => {
                libc::EINVAL
            }
        }
    }

    pub fn from_raw_os_error(code: i32) -> Option<Self> {
        match code {
            libc::EPERM => Some(Self::Eperm),
            libc::EACCES => Some(Self::Eacces),
            libc::ENOENT => Some(Self::Enoent),
            libc::EINVAL => Some(Self::Einval),
            libc::ENOSPC => Some(Self::Enospc),
            libc::EROFS => Some(Self::Erofs),
            libc::ECONNREFUSED => Some(Self::Econnrefused),
            libc::ENXIO => Some(Self::Enxio),
            libc::EWOULDBLOCK => Some(Self::Ewouldblock),
            libc::EFAULT => Some(Self::Efault),
            libc::EBUSY => Some(Self::Ebusy),
            libc::EXDEV => Some(Self::Exdev),
            libc::EPIPE => Some(Self::Epipe),
            libc::ENODATA => Some(Self::Enodata),
            libc::EPROTO => Some(Self::Eproto),
            _ => None,
        }
    }
}

impl fmt::Display for Errno {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// see: cans/02-plugin-model.md#Unified-verdict
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownErrno(pub String);

impl fmt::Display for UnknownErrno {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown errno `{}`", self.0)
    }
}

impl std::error::Error for UnknownErrno {}

impl std::str::FromStr for Errno {
    type Err = UnknownErrno;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "EPERM" => Ok(Self::Eperm),
            "EACCES" => Ok(Self::Eacces),
            "ENOENT" => Ok(Self::Enoent),
            "EINVAL" => Ok(Self::Einval),
            "ENOSPC" => Ok(Self::Enospc),
            "EROFS" => Ok(Self::Erofs),
            "ECONNREFUSED" => Ok(Self::Econnrefused),
            "ENXIO" => Ok(Self::Enxio),
            "EWOULDBLOCK" => Ok(Self::Ewouldblock),
            "EFAULT" => Ok(Self::Efault),
            "EBUSY" => Ok(Self::Ebusy),
            "EXDEV" => Ok(Self::Exdev),
            "EPIPE" => Ok(Self::Epipe),
            "ENODATA" => Ok(Self::Enodata),
            "EPROTO" => Ok(Self::Eproto),
            "EPOLICY" => Ok(Self::Epolicy),
            "ETOOLQUOTA" => Ok(Self::Etoolquota),
            "EWXVIOLATION" => Ok(Self::Ewxviolation),
            "EConfig" => Ok(Self::Econfig),
            "ESecurityViolation" => Ok(Self::Esecurityviolation),
            "EUnknownKind" => Ok(Self::Eunknownkind),
            "EClockRegress" => Ok(Self::Eclockregress),
            "PathTraversal" => Ok(Self::PathTraversal),
            _ => Err(UnknownErrno(value.to_string())),
        }
    }
}
