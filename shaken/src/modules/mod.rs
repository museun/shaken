macro_rules! export {
    ($($m:tt),*) => {
        $(
            mod $m;
            pub use self::$m::*;
        )*

        pub const MODULES: &[&str] = &[
            $( $m::NAME, )*
        ];
    };
}

export!(
    builtin,     //
    shakespeare, //
    invest,      //
    twitchpoll,  //
    currentsong, //
    rust         //
);
