use std::io;

/// The owner authority of one setup host, independent of origin and route prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupScope {
    Idp,
    Management,
    Storage,
    Unified,
}

impl SetupScope {
    pub fn from_owners(owners: &[&str]) -> io::Result<Self> {
        for (index, owner) in owners.iter().enumerate() {
            if !["idp", "management", "storage"].contains(owner) || owners[..index].contains(owner)
            {
                return Err(io::Error::other(
                    "setup scope has unknown or duplicate owners",
                ));
            }
        }
        match owners {
            ["idp"] => Ok(Self::Idp),
            ["management"] => Ok(Self::Management),
            ["storage"] => Ok(Self::Storage),
            [_, _, _] => Ok(Self::Unified),
            _ => Err(io::Error::other(
                "setup scope must be one owner or the complete unified host",
            )),
        }
    }

    pub fn owners(self) -> &'static [&'static str] {
        match self {
            Self::Idp => &["idp"],
            Self::Management => &["management"],
            Self::Storage => &["storage"],
            Self::Unified => &["idp", "management", "storage"],
        }
    }
}
