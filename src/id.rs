macro_rules! id {
    ($name:ident, $field:ident, $ty:ty) => {
        id!($name);

        // These methods call `Option::expect` because we treat a situation where an ID reference
        // can't be found as an invariant. See `Database::check_consistency`.
        impl $name {
            pub fn load(self, database: &$crate::Database) -> &$ty {
                database.$field.get(&self).expect(&format!(
                    "{} {} not found",
                    stringify!($name),
                    self
                ))
            }

            pub fn load_mut(self, database: &mut $crate::Database) -> &mut $ty {
                database.$field.get_mut(&self).expect(&format!(
                    "{} {} not found",
                    stringify!($name),
                    self
                ))
            }
        }
    };

    ($name:ident) => {
        #[derive(
            Clone,
            Copy,
            Default,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            ::serde::Deserialize,
            ::serde::Serialize,
        )]
        #[repr(transparent)]
        #[serde(transparent)]
        pub struct $name(pub ::uuid::Uuid);

        impl $name {
            pub fn new() -> $name {
                $name(::uuid::Uuid::new_v4())
            }
        }

        impl ::std::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = ::uuid::Error;

            fn from_str(s: &str) -> Result<$name, ::uuid::Error> {
                ::std::str::FromStr::from_str(s).map($name)
            }
        }
    };
}

id!(BallparkId);
id!(GameId);
id!(PlayerId, players, crate::Player);
id!(TeamId, teams, crate::Team);
