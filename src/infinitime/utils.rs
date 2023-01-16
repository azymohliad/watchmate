pub struct ScopeGuard<F: Fn() -> ()>(F);

impl<F: Fn() -> ()> ScopeGuard<F> {
    pub fn new(f: F) -> Self { Self(f) }
}

impl<F: Fn() -> ()> Drop for ScopeGuard<F> {
    fn drop(&mut self) { self.0(); }
}


/// Declare enum that is convertible from a primitive
/// type via automatic TryFrom implementation
macro_rules! value_enum {
    (
        $(#[$attr:meta])*
        $visibility:vis enum $name:ident::<$type:ty> {
            $( $variant:ident = $value:literal ),*
        }
    ) => {
        $(#[$attr])*
        $visibility enum $name {
            $(
                $variant = $value
            ),*
        }
        
        impl TryFrom<$type> for $name {
            type Error = anyhow::Error;
            
            fn try_from(v: $type) -> Result<Self, Self::Error> {
                match v {
                    $(x if x == Self::$variant as $type => Ok(Self::$variant),)* 
                    _ => Err(anyhow::anyhow!("Invalid enum value: {}", v)),
                }
            }
        }
    };
}

pub(crate) use value_enum;
